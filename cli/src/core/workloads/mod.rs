use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::{Context, Result};
use async_trait::async_trait;
use tokio::{fs, process::Command};

use crate::core::{
    models::{
        ManagedWorkload, ManagedWorkloadBackend, ManagedWorkloadCapabilities, ManagedWorkloadSpec,
        ManagedWorkloadState,
    },
    packages::which,
    platform::OsFamily,
    services::{is_systemd_available, ServiceManager, ServiceUnit},
};

const OWNED_MARKER: &str = "# postlab-managed: true";
const SPEC_MARKER: &str = "# postlab-spec: ";

#[derive(Debug, Clone)]
pub struct ManagedWorkloadPaths {
    pub quadlet_dir: PathBuf,
    pub docker_root: PathBuf,
    pub systemd_dir: PathBuf,
}

impl Default for ManagedWorkloadPaths {
    fn default() -> Self {
        Self {
            quadlet_dir: PathBuf::from("/etc/containers/systemd"),
            docker_root: PathBuf::from("/etc/postlab/workloads"),
            systemd_dir: PathBuf::from("/etc/systemd/system"),
        }
    }
}

#[async_trait]
pub trait ManagedWorkloadManager: Send + Sync {
    async fn capabilities(&self) -> ManagedWorkloadCapabilities;
    async fn list_workloads(&self) -> Result<Vec<ManagedWorkload>>;
    async fn get_workload(&self, name: &str) -> Result<Option<ManagedWorkload>>;
    async fn create_workload(&self, spec: ManagedWorkloadSpec) -> Result<ManagedWorkload>;
    async fn update_workload(
        &self,
        name: &str,
        spec: ManagedWorkloadSpec,
    ) -> Result<ManagedWorkload>;
    async fn delete_workload(&self, name: &str) -> Result<()>;
    async fn start_workload(&self, name: &str) -> Result<()>;
    async fn stop_workload(&self, name: &str) -> Result<()>;
    async fn restart_workload(&self, name: &str) -> Result<()>;
    async fn enable_workload(&self, name: &str) -> Result<()>;
    async fn disable_workload(&self, name: &str) -> Result<()>;
}

pub struct DefaultManagedWorkloadManager {
    os: OsFamily,
    services: Arc<dyn ServiceManager>,
    paths: ManagedWorkloadPaths,
    systemctl_bin: PathBuf,
    docker_bin: PathBuf,
    engine_override: Option<String>,
    systemd_override: Option<bool>,
}

impl DefaultManagedWorkloadManager {
    pub fn detect(os: OsFamily, services: Arc<dyn ServiceManager>) -> Self {
        Self {
            os,
            services,
            paths: ManagedWorkloadPaths::default(),
            systemctl_bin: resolve_bin("systemctl")
                .unwrap_or_else(|| PathBuf::from("/usr/bin/systemctl")),
            docker_bin: resolve_bin("docker").unwrap_or_else(|| PathBuf::from("/usr/bin/docker")),
            engine_override: None,
            systemd_override: None,
        }
    }

    #[cfg(test)]
    fn with_overrides(
        os: OsFamily,
        services: Arc<dyn ServiceManager>,
        paths: ManagedWorkloadPaths,
        systemctl_bin: PathBuf,
        docker_bin: PathBuf,
        engine_override: Option<String>,
        systemd_override: Option<bool>,
    ) -> Self {
        Self {
            os,
            services,
            paths,
            systemctl_bin,
            docker_bin,
            engine_override,
            systemd_override,
        }
    }

    fn detect_engine(&self) -> Option<String> {
        if let Some(engine) = &self.engine_override {
            return Some(engine.clone());
        }
        if which("docker") {
            return Some("docker".to_string());
        }
        if which("podman") {
            return Some("podman".to_string());
        }
        None
    }

    fn systemd_available(&self) -> bool {
        self.systemd_override.unwrap_or_else(is_systemd_available)
    }

    fn selected_backend(&self) -> Option<ManagedWorkloadBackend> {
        match self.detect_engine().as_deref() {
            Some("podman") => Some(ManagedWorkloadBackend::PodmanQuadlet),
            Some("docker") => Some(ManagedWorkloadBackend::DockerComposeSystemd),
            _ => None,
        }
    }

    fn validate_spec(spec: &ManagedWorkloadSpec) -> Result<()> {
        if spec.name.trim().is_empty() {
            anyhow::bail!("Workload name cannot be empty");
        }
        if spec.image.trim().is_empty() {
            anyhow::bail!("Workload image cannot be empty");
        }

        match spec.restart_policy.as_str() {
            "always" | "unless-stopped" | "on-failure" | "no" => {}
            other => anyhow::bail!("Unsupported restart policy: {}", other),
        }

        if let Some(command) = &spec.command {
            if command.iter().any(|arg| arg.trim().is_empty()) {
                anyhow::bail!("Command entries cannot be empty");
            }
        }

        for (key, value) in &spec.env {
            if key.trim().is_empty()
                || key.contains('=')
                || key.contains('\n')
                || value.contains('\n')
            {
                anyhow::bail!("Invalid environment variable '{}'", key);
            }
        }

        for port in &spec.ports {
            if port.trim().is_empty() || port.contains('\n') {
                anyhow::bail!("Invalid port mapping '{}'", port);
            }
        }

        for volume in &spec.volumes {
            if volume.trim().is_empty() || volume.contains('\n') {
                anyhow::bail!("Invalid volume mapping '{}'", volume);
            }
        }

        Ok(())
    }

    fn normalize_spec(mut spec: ManagedWorkloadSpec) -> ManagedWorkloadSpec {
        spec.name = spec.name.trim().to_string();
        spec.image = spec.image.trim().to_string();
        spec.env = spec
            .env
            .into_iter()
            .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
            .filter(|(k, _)| !k.is_empty())
            .collect();
        spec.ports = spec
            .ports
            .into_iter()
            .map(|p| p.trim().to_string())
            .filter(|p| !p.is_empty())
            .collect();
        spec.volumes = spec
            .volumes
            .into_iter()
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
            .collect();
        spec.command = spec.command.and_then(|cmd| {
            let trimmed: Vec<String> = cmd
                .into_iter()
                .map(|entry| entry.trim().to_string())
                .filter(|entry| !entry.is_empty())
                .collect();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            }
        });
        if spec.restart_policy.trim().is_empty() {
            spec.restart_policy = "unless-stopped".to_string();
        } else {
            spec.restart_policy = spec.restart_policy.trim().to_string();
        }
        spec
    }

    fn slug(name: &str) -> String {
        let mut slug = String::new();
        let mut last_dash = false;

        for ch in name.chars() {
            if ch.is_ascii_alphanumeric() {
                slug.push(ch.to_ascii_lowercase());
                last_dash = false;
            } else if matches!(ch, '-' | '_' | ' ' | '.') {
                if !slug.is_empty() && !last_dash {
                    slug.push('-');
                    last_dash = true;
                }
            }
        }

        slug.trim_matches('-').to_string()
    }

    fn unit_name(slug: &str) -> String {
        format!("postlab-{}.service", slug)
    }

    fn quadlet_path(&self, slug: &str) -> PathBuf {
        self.paths
            .quadlet_dir
            .join(format!("postlab-{}.container", slug))
    }

    fn docker_dir(&self, slug: &str) -> PathBuf {
        self.paths.docker_root.join(slug)
    }

    fn compose_path(&self, slug: &str) -> PathBuf {
        self.docker_dir(slug).join("compose.yml")
    }

    fn docker_unit_path(&self, slug: &str) -> PathBuf {
        self.paths.systemd_dir.join(Self::unit_name(slug))
    }

    async fn daemon_reload(&self) -> Result<()> {
        let output = Command::new(&self.systemctl_bin)
            .arg("daemon-reload")
            .output()
            .await
            .context("Failed to run systemctl daemon-reload")?;

        if !output.status.success() {
            anyhow::bail!(
                "systemctl daemon-reload failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        Ok(())
    }

    async fn write_file(path: &Path, content: &str) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await?;
        }
        fs::write(path, content).await?;
        Ok(())
    }

    async fn remove_if_exists(path: &Path) -> Result<()> {
        match fs::remove_file(path).await {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(err) => Err(err.into()),
        }
    }

    fn render_header(spec: &ManagedWorkloadSpec) -> Result<String> {
        let spec_json = serde_json::to_string(spec)?;
        Ok(format!("{}\n{}{}\n", OWNED_MARKER, SPEC_MARKER, spec_json))
    }

    fn render_quadlet(&self, spec: &ManagedWorkloadSpec) -> Result<String> {
        let slug = Self::slug(&spec.name);
        let container_name = format!("postlab-{}", slug);
        let mut out = Self::render_header(spec)?;
        out.push_str("[Unit]\n");
        out.push_str(&format!("Description=Postlab workload {}\n\n", spec.name));
        out.push_str("[Container]\n");
        out.push_str(&format!("Image={}\n", spec.image));
        out.push_str(&format!("ContainerName={}\n", container_name));
        for port in &spec.ports {
            out.push_str(&format!("PublishPort={}\n", port));
        }
        for volume in &spec.volumes {
            out.push_str(&format!("Volume={}\n", volume));
        }
        for (key, value) in &spec.env {
            out.push_str(&format!("Environment={}={}\n", key, value));
        }
        if let Some(command) = &spec.command {
            out.push_str(&format!("Exec={}\n", systemd_command(command)));
        }
        out.push('\n');
        out.push_str("[Service]\n");
        out.push_str(&format!(
            "Restart={}\n\n",
            systemd_restart_policy(&spec.restart_policy)
        ));
        out.push_str("[Install]\nWantedBy=multi-user.target\n");
        Ok(out)
    }

    fn render_compose(&self, spec: &ManagedWorkloadSpec) -> Result<String> {
        let slug = Self::slug(&spec.name);
        let mut out = Self::render_header(spec)?;
        out.push_str(&format!("name: postlab-{}\n", slug));
        out.push_str("services:\n");
        out.push_str("  app:\n");
        out.push_str(&format!("    image: {}\n", yaml_string(&spec.image)));
        out.push_str(&format!(
            "    container_name: {}\n",
            yaml_string(&format!("postlab-{}", slug))
        ));
        out.push_str(&format!(
            "    restart: {}\n",
            yaml_string(&spec.restart_policy)
        ));
        if let Some(command) = &spec.command {
            out.push_str("    command:\n");
            for arg in command {
                out.push_str(&format!("      - {}\n", yaml_string(arg)));
            }
        }
        if !spec.env.is_empty() {
            out.push_str("    environment:\n");
            for (key, value) in &spec.env {
                out.push_str(&format!("      {}: {}\n", key, yaml_string(value)));
            }
        }
        if !spec.ports.is_empty() {
            out.push_str("    ports:\n");
            for port in &spec.ports {
                out.push_str(&format!("      - {}\n", yaml_string(port)));
            }
        }
        if !spec.volumes.is_empty() {
            out.push_str("    volumes:\n");
            for volume in &spec.volumes {
                out.push_str(&format!("      - {}\n", yaml_string(volume)));
            }
        }
        Ok(out)
    }

    fn render_docker_service(&self, spec: &ManagedWorkloadSpec) -> String {
        let slug = Self::slug(&spec.name);
        let compose_path = self.compose_path(&slug);
        let working_dir = self.docker_dir(&slug);
        let docker_bin = self.docker_bin.display();
        format!(
            "{owned}\n[Unit]\nDescription=Postlab workload {name}\nRequires=docker.service\nAfter=docker.service network-online.target\nWants=network-online.target\n\n[Service]\nType=oneshot\nRemainAfterExit=yes\nWorkingDirectory={working_dir}\nExecStart={docker_bin} compose -f {compose_path} up -d\nExecStop={docker_bin} compose -f {compose_path} down\nExecReload={docker_bin} compose -f {compose_path} up -d\nTimeoutStartSec=0\n\n[Install]\nWantedBy=multi-user.target\n",
            owned = OWNED_MARKER,
            name = spec.name,
            working_dir = working_dir.display(),
            docker_bin = docker_bin,
            compose_path = compose_path.display(),
        )
    }

    async fn service_index(&self) -> Result<HashMap<String, ServiceUnit>> {
        Ok(self
            .services
            .list_services()
            .await?
            .into_iter()
            .map(|unit| (unit.name.clone(), unit))
            .collect())
    }

    fn service_state(unit: Option<&ServiceUnit>, unit_exists: bool) -> ManagedWorkloadState {
        if !unit_exists {
            return ManagedWorkloadState::NotInstalled;
        }

        match unit {
            Some(unit) if unit.active_state == "active" => ManagedWorkloadState::Running,
            Some(unit) if unit.active_state == "inactive" => ManagedWorkloadState::Stopped,
            Some(unit) if unit.active_state == "failed" => ManagedWorkloadState::Failed,
            Some(_) => ManagedWorkloadState::Unknown,
            None => ManagedWorkloadState::Unknown,
        }
    }

    fn is_owned(content: &str) -> bool {
        content.lines().any(|line| line.trim() == OWNED_MARKER)
    }

    fn spec_from_content(content: &str) -> Result<ManagedWorkloadSpec> {
        let raw = content
            .lines()
            .find_map(|line| line.trim().strip_prefix(SPEC_MARKER))
            .context("Missing Postlab workload spec metadata")?;
        Ok(serde_json::from_str(raw)?)
    }

    async fn list_podman_workloads(&self) -> Result<Vec<ManagedWorkload>> {
        let services = self.service_index().await.unwrap_or_default();
        let mut workloads = Vec::new();

        let mut entries = match fs::read_dir(&self.paths.quadlet_dir).await {
            Ok(entries) => entries,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(workloads),
            Err(err) => return Err(err.into()),
        };

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("container") {
                continue;
            }
            if !path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default()
                .starts_with("postlab-")
            {
                continue;
            }

            let content = fs::read_to_string(&path).await?;
            if !Self::is_owned(&content) {
                continue;
            }

            let spec = Self::spec_from_content(&content)?;
            let stem = path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or_default();
            let unit_name = format!("{}.service", stem);
            workloads.push(ManagedWorkload {
                name: spec.name.clone(),
                backend: ManagedWorkloadBackend::PodmanQuadlet,
                unit_name: unit_name.clone(),
                engine: "podman".to_string(),
                image: spec.image.clone(),
                ports_summary: join_or_dash(&spec.ports),
                status: Self::service_state(services.get(&unit_name), true),
                owned_by_postlab: true,
                spec_path: path.display().to_string(),
                compose_path: None,
                spec: Some(spec),
            });
        }

        workloads.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(workloads)
    }

    async fn list_docker_workloads(&self) -> Result<Vec<ManagedWorkload>> {
        let services = self.service_index().await.unwrap_or_default();
        let mut workloads = Vec::new();

        let mut entries = match fs::read_dir(&self.paths.docker_root).await {
            Ok(entries) => entries,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(workloads),
            Err(err) => return Err(err.into()),
        };

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if !entry.file_type().await?.is_dir() {
                continue;
            }

            let compose_path = path.join("compose.yml");
            let content = match fs::read_to_string(&compose_path).await {
                Ok(content) => content,
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => continue,
                Err(err) => return Err(err.into()),
            };
            if !Self::is_owned(&content) {
                continue;
            }

            let spec = Self::spec_from_content(&content)?;
            let slug = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default();
            let unit_name = Self::unit_name(slug);
            let unit_path = self.docker_unit_path(slug);
            workloads.push(ManagedWorkload {
                name: spec.name.clone(),
                backend: ManagedWorkloadBackend::DockerComposeSystemd,
                unit_name: unit_name.clone(),
                engine: "docker".to_string(),
                image: spec.image.clone(),
                ports_summary: join_or_dash(&spec.ports),
                status: Self::service_state(services.get(&unit_name), unit_path.exists()),
                owned_by_postlab: true,
                spec_path: unit_path.display().to_string(),
                compose_path: Some(compose_path.display().to_string()),
                spec: Some(spec),
            });
        }

        workloads.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(workloads)
    }

    async fn resolved_workload(&self, name: &str) -> Result<ManagedWorkload> {
        self.get_workload(name)
            .await?
            .with_context(|| format!("Workload '{}' not found", name))
    }
}

#[async_trait]
impl ManagedWorkloadManager for DefaultManagedWorkloadManager {
    async fn capabilities(&self) -> ManagedWorkloadCapabilities {
        if !self.os.is_linux() {
            return ManagedWorkloadCapabilities {
                supported: false,
                engine: self.detect_engine(),
                backend: self.selected_backend(),
                reason: Some("Workloads are only available on Linux in v1.".to_string()),
            };
        }

        if !self.systemd_available() {
            return ManagedWorkloadCapabilities {
                supported: false,
                engine: self.detect_engine(),
                backend: self.selected_backend(),
                reason: Some("Workloads require systemd on the host.".to_string()),
            };
        }

        let engine = self.detect_engine();
        let backend = self.selected_backend();
        let supported = backend.is_some();
        ManagedWorkloadCapabilities {
            supported,
            engine: engine.clone(),
            backend,
            reason: if supported {
                None
            } else {
                Some("Docker or Podman was not detected.".to_string())
            },
        }
    }

    async fn list_workloads(&self) -> Result<Vec<ManagedWorkload>> {
        let capabilities = self.capabilities().await;
        if !capabilities.supported {
            return Ok(Vec::new());
        }

        match capabilities.backend {
            Some(ManagedWorkloadBackend::PodmanQuadlet) => self.list_podman_workloads().await,
            Some(ManagedWorkloadBackend::DockerComposeSystemd) => {
                self.list_docker_workloads().await
            }
            None => Ok(Vec::new()),
        }
    }

    async fn get_workload(&self, name: &str) -> Result<Option<ManagedWorkload>> {
        let slug = Self::slug(name);
        Ok(self
            .list_workloads()
            .await?
            .into_iter()
            .find(|workload| Self::slug(&workload.name) == slug))
    }

    async fn create_workload(&self, spec: ManagedWorkloadSpec) -> Result<ManagedWorkload> {
        let capabilities = self.capabilities().await;
        if !capabilities.supported {
            anyhow::bail!(
                "{}",
                capabilities
                    .reason
                    .unwrap_or_else(|| "Workloads are unavailable.".to_string())
            );
        }

        let spec = Self::normalize_spec(spec);
        Self::validate_spec(&spec)?;
        let slug = Self::slug(&spec.name);
        if slug.is_empty() {
            anyhow::bail!("Workload name must contain at least one ASCII letter or number");
        }
        if self.get_workload(&spec.name).await?.is_some() {
            anyhow::bail!("Workload '{}' already exists", spec.name);
        }

        match capabilities.backend {
            Some(ManagedWorkloadBackend::PodmanQuadlet) => {
                let path = self.quadlet_path(&slug);
                let content = self.render_quadlet(&spec)?;
                Self::write_file(&path, &content).await?;
            }
            Some(ManagedWorkloadBackend::DockerComposeSystemd) => {
                let compose_path = self.compose_path(&slug);
                let unit_path = self.docker_unit_path(&slug);
                let compose = self.render_compose(&spec)?;
                let unit = self.render_docker_service(&spec);
                Self::write_file(&compose_path, &compose).await?;
                Self::write_file(&unit_path, &unit).await?;
            }
            None => anyhow::bail!("No supported workload backend is available"),
        }

        self.daemon_reload().await?;
        self.resolved_workload(&spec.name).await
    }

    async fn update_workload(
        &self,
        name: &str,
        spec: ManagedWorkloadSpec,
    ) -> Result<ManagedWorkload> {
        let existing = self.resolved_workload(name).await?;
        let spec = Self::normalize_spec(spec);
        Self::validate_spec(&spec)?;

        if Self::slug(&existing.name) != Self::slug(&spec.name) {
            anyhow::bail!(
                "Renaming workloads is not supported in v1; recreate the workload instead"
            );
        }

        let capabilities = self.capabilities().await;
        if Some(existing.backend.clone()) != capabilities.backend {
            anyhow::bail!(
                "Backend switching is not supported in v1; recreate the workload instead"
            );
        }

        match existing.backend {
            ManagedWorkloadBackend::PodmanQuadlet => {
                let path = self.quadlet_path(&Self::slug(&existing.name));
                let content = self.render_quadlet(&spec)?;
                Self::write_file(&path, &content).await?;
            }
            ManagedWorkloadBackend::DockerComposeSystemd => {
                let slug = Self::slug(&existing.name);
                let compose = self.render_compose(&spec)?;
                let unit = self.render_docker_service(&spec);
                Self::write_file(&self.compose_path(&slug), &compose).await?;
                Self::write_file(&self.docker_unit_path(&slug), &unit).await?;
            }
        }

        self.daemon_reload().await?;
        if existing.status == ManagedWorkloadState::Running {
            self.restart_workload(&existing.name).await?;
        }
        self.resolved_workload(&existing.name).await
    }

    async fn delete_workload(&self, name: &str) -> Result<()> {
        let workload = self.resolved_workload(name).await?;
        let _ = self.services.stop(&workload.unit_name).await;
        let _ = self.services.disable(&workload.unit_name).await;

        match workload.backend {
            ManagedWorkloadBackend::PodmanQuadlet => {
                Self::remove_if_exists(&self.quadlet_path(&Self::slug(&workload.name))).await?;
            }
            ManagedWorkloadBackend::DockerComposeSystemd => {
                let slug = Self::slug(&workload.name);
                let compose_path = self.compose_path(&slug);
                let docker_dir = self.docker_dir(&slug);
                Self::remove_if_exists(&self.docker_unit_path(&slug)).await?;
                Self::remove_if_exists(&compose_path).await?;
                match fs::remove_dir(&docker_dir).await {
                    Ok(()) => {}
                    Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
                    Err(err) if err.kind() == std::io::ErrorKind::DirectoryNotEmpty => {}
                    Err(err) => return Err(err.into()),
                }
            }
        }

        self.daemon_reload().await?;
        Ok(())
    }

    async fn start_workload(&self, name: &str) -> Result<()> {
        let workload = self.resolved_workload(name).await?;
        self.services.start(&workload.unit_name).await
    }

    async fn stop_workload(&self, name: &str) -> Result<()> {
        let workload = self.resolved_workload(name).await?;
        self.services.stop(&workload.unit_name).await
    }

    async fn restart_workload(&self, name: &str) -> Result<()> {
        let workload = self.resolved_workload(name).await?;
        self.services.restart(&workload.unit_name).await
    }

    async fn enable_workload(&self, name: &str) -> Result<()> {
        let workload = self.resolved_workload(name).await?;
        self.services.enable(&workload.unit_name).await
    }

    async fn disable_workload(&self, name: &str) -> Result<()> {
        let workload = self.resolved_workload(name).await?;
        self.services.disable(&workload.unit_name).await
    }
}

fn resolve_bin(bin: &str) -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    std::env::split_paths(&path_var)
        .map(|dir| dir.join(bin))
        .find(|candidate| candidate.is_file())
}

fn yaml_string(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

fn systemd_restart_policy(value: &str) -> &'static str {
    match value {
        "always" | "unless-stopped" => "always",
        "on-failure" => "on-failure",
        _ => "no",
    }
}

fn systemd_command(args: &[String]) -> String {
    args.iter()
        .map(|arg| {
            if arg
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || "-_./:".contains(ch))
            {
                arg.clone()
            } else {
                format!("\"{}\"", arg.replace('\\', "\\\\").replace('"', "\\\""))
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn join_or_dash(items: &[String]) -> String {
    if items.is_empty() {
        "-".to_string()
    } else {
        items.join(", ")
    }
}

#[cfg(test)]
mod tests {
    use std::{
        path::PathBuf,
        sync::{Arc, Mutex},
        time::{SystemTime, UNIX_EPOCH},
    };

    use anyhow::Result;
    use async_trait::async_trait;

    use crate::core::{
        platform::OsFamily,
        services::{ServiceManager, ServiceUnit},
    };

    use super::{
        DefaultManagedWorkloadManager, ManagedWorkloadManager, ManagedWorkloadPaths,
        ManagedWorkloadSpec,
    };

    #[derive(Default)]
    struct MockServiceManager {
        services: Mutex<Vec<ServiceUnit>>,
        calls: Mutex<Vec<String>>,
    }

    impl MockServiceManager {
        fn with_services(services: Vec<ServiceUnit>) -> Self {
            Self {
                services: Mutex::new(services),
                calls: Mutex::new(Vec::new()),
            }
        }
    }

    #[async_trait]
    impl ServiceManager for MockServiceManager {
        async fn list_services(&self) -> Result<Vec<ServiceUnit>> {
            Ok(self.services.lock().unwrap().clone())
        }

        async fn start(&self, name: &str) -> Result<()> {
            self.calls.lock().unwrap().push(format!("start:{name}"));
            Ok(())
        }

        async fn stop(&self, name: &str) -> Result<()> {
            self.calls.lock().unwrap().push(format!("stop:{name}"));
            Ok(())
        }

        async fn restart(&self, name: &str) -> Result<()> {
            self.calls.lock().unwrap().push(format!("restart:{name}"));
            Ok(())
        }

        async fn enable(&self, name: &str) -> Result<()> {
            self.calls.lock().unwrap().push(format!("enable:{name}"));
            Ok(())
        }

        async fn disable(&self, name: &str) -> Result<()> {
            self.calls.lock().unwrap().push(format!("disable:{name}"));
            Ok(())
        }
    }

    fn temp_paths(prefix: &str) -> (PathBuf, ManagedWorkloadPaths, PathBuf, PathBuf) {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("postlab-{prefix}-{unique}"));
        let paths = ManagedWorkloadPaths {
            quadlet_dir: root.join("quadlet"),
            docker_root: root.join("docker"),
            systemd_dir: root.join("systemd"),
        };
        let systemctl = root.join("bin/systemctl");
        let docker = root.join("bin/docker");
        (root, paths, systemctl, docker)
    }

    fn manager_with_engine(
        engine: &str,
        services: Arc<dyn ServiceManager>,
        paths: ManagedWorkloadPaths,
        systemctl_bin: PathBuf,
        docker_bin: PathBuf,
    ) -> DefaultManagedWorkloadManager {
        DefaultManagedWorkloadManager::with_overrides(
            OsFamily::Debian,
            services,
            paths,
            systemctl_bin,
            docker_bin,
            Some(engine.to_string()),
            Some(true),
        )
    }

    fn sample_spec() -> ManagedWorkloadSpec {
        ManagedWorkloadSpec {
            name: "Demo API".to_string(),
            image: "ghcr.io/example/demo:latest".to_string(),
            command: Some(vec!["demo".to_string(), "--serve".to_string()]),
            env: vec![("APP_ENV".to_string(), "prod".to_string())],
            ports: vec!["8080:8080".to_string()],
            volumes: vec!["/data:/data".to_string()],
            restart_policy: "unless-stopped".to_string(),
        }
    }

    #[tokio::test]
    async fn renders_quadlet_with_expected_fields() {
        let (_root, paths, systemctl, docker) = temp_paths("quadlet-render");
        let services = Arc::new(MockServiceManager::default());
        let manager = manager_with_engine("podman", services, paths, systemctl, docker);

        let rendered = manager.render_quadlet(&sample_spec()).unwrap();
        assert!(rendered.contains(super::OWNED_MARKER));
        assert!(rendered.contains("Image=ghcr.io/example/demo:latest"));
        assert!(rendered.contains("ContainerName=postlab-demo-api"));
        assert!(rendered.contains("PublishPort=8080:8080"));
        assert!(rendered.contains("Environment=APP_ENV=prod"));
        assert!(rendered.contains("Volume=/data:/data"));
        assert!(rendered.contains("Exec=demo --serve"));
    }

    #[tokio::test]
    async fn renders_compose_and_systemd_unit_with_expected_fields() {
        let (_root, paths, systemctl, docker) = temp_paths("docker-render");
        let services = Arc::new(MockServiceManager::default());
        let manager = manager_with_engine("docker", services, paths, systemctl, docker.clone());

        let compose = manager.render_compose(&sample_spec()).unwrap();
        assert!(compose.contains(super::OWNED_MARKER));
        assert!(compose.contains("name: postlab-demo-api"));
        assert!(compose.contains("image: \"ghcr.io/example/demo:latest\""));
        assert!(compose.contains("restart: \"unless-stopped\""));
        assert!(compose.contains("- \"8080:8080\""));

        let unit = manager.render_docker_service(&sample_spec());
        assert!(unit.contains("ExecStart="));
        assert!(unit.contains("compose.yml up -d"));
        assert!(unit.contains(docker.to_string_lossy().as_ref()));
    }

    #[tokio::test]
    async fn lists_postlab_owned_workloads_from_backend_files() {
        let (root, paths, systemctl, docker) = temp_paths("list");
        std::fs::create_dir_all(&paths.quadlet_dir).unwrap();
        std::fs::create_dir_all(paths.docker_root.join("demo-api")).unwrap();
        std::fs::create_dir_all(&paths.systemd_dir).unwrap();

        let podman_services = vec![ServiceUnit {
            name: "postlab-demo-api.service".to_string(),
            description: "demo".to_string(),
            load_state: "loaded".to_string(),
            active_state: "active".to_string(),
            sub_state: "running".to_string(),
        }];
        let services = Arc::new(MockServiceManager::with_services(podman_services));

        let podman = manager_with_engine(
            "podman",
            services.clone(),
            paths.clone(),
            systemctl.clone(),
            docker.clone(),
        );
        std::fs::write(
            paths.quadlet_dir.join("postlab-demo-api.container"),
            podman.render_quadlet(&sample_spec()).unwrap(),
        )
        .unwrap();

        let listed = podman.list_workloads().await.unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].name, "Demo API");
        assert_eq!(listed[0].status.label(), "Running");

        let docker_manager =
            manager_with_engine("docker", services, paths.clone(), systemctl, docker);
        std::fs::write(
            paths.docker_root.join("demo-api/compose.yml"),
            docker_manager.render_compose(&sample_spec()).unwrap(),
        )
        .unwrap();
        std::fs::write(
            paths.systemd_dir.join("postlab-demo-api.service"),
            docker_manager.render_docker_service(&sample_spec()),
        )
        .unwrap();

        let listed = docker_manager.list_workloads().await.unwrap();
        assert_eq!(listed.len(), 1);
        assert!(listed[0]
            .compose_path
            .as_deref()
            .unwrap()
            .ends_with("compose.yml"));

        let _ = std::fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn create_update_and_delete_manage_files_and_reload_systemd() {
        let (root, paths, systemctl, docker) = temp_paths("crud");
        std::fs::create_dir_all(systemctl.parent().unwrap()).unwrap();
        std::fs::write(
            &systemctl,
            "#!/bin/sh\nprintf '%s\\n' \"$@\" >> \"$(dirname \"$0\")/systemctl.log\"\n",
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&systemctl).unwrap().permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&systemctl, perms).unwrap();
        }

        std::fs::create_dir_all(&paths.systemd_dir).unwrap();
        let services = Arc::new(MockServiceManager::default());
        let manager = manager_with_engine(
            "docker",
            services.clone(),
            paths.clone(),
            systemctl.clone(),
            docker,
        );

        let created = manager.create_workload(sample_spec()).await.unwrap();
        assert_eq!(created.unit_name, "postlab-demo-api.service");
        assert!(paths.docker_root.join("demo-api/compose.yml").exists());
        assert!(paths.systemd_dir.join("postlab-demo-api.service").exists());

        let mut updated_spec = sample_spec();
        updated_spec.image = "ghcr.io/example/demo:v2".to_string();
        manager
            .update_workload("Demo API", updated_spec)
            .await
            .unwrap();
        let compose =
            std::fs::read_to_string(paths.docker_root.join("demo-api/compose.yml")).unwrap();
        assert!(compose.contains("ghcr.io/example/demo:v2"));

        manager.delete_workload("Demo API").await.unwrap();
        assert!(!paths.systemd_dir.join("postlab-demo-api.service").exists());
        assert!(!paths.docker_root.join("demo-api/compose.yml").exists());

        let calls = services.calls.lock().unwrap().clone();
        assert!(calls
            .iter()
            .any(|call| call == "stop:postlab-demo-api.service"));
        assert!(calls
            .iter()
            .any(|call| call == "disable:postlab-demo-api.service"));

        let log =
            std::fs::read_to_string(systemctl.parent().unwrap().join("systemctl.log")).unwrap();
        assert!(log.lines().count() >= 3);

        let _ = std::fs::remove_dir_all(root);
    }
}
