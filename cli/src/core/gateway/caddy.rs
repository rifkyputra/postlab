use super::GatewayManager;
use crate::core::models::Route;
use anyhow::Result;
use async_trait::async_trait;
use tokio::fs;

const CADDYFILE: &str = "/etc/caddy/Caddyfile";
const CADDYFILE_FALLBACK: &str = "/usr/local/etc/caddy/Caddyfile"; // macOS brew

pub struct CaddyManager;

impl CaddyManager {
    fn caddyfile_path() -> &'static str {
        if std::path::Path::new(CADDYFILE).exists() {
            CADDYFILE
        } else {
            CADDYFILE_FALLBACK
        }
    }

    async fn read_caddyfile() -> Result<String> {
        let path = Self::caddyfile_path();
        Ok(fs::read_to_string(path).await.unwrap_or_default())
    }

    async fn write_caddyfile(content: &str) -> Result<()> {
        let path = Self::caddyfile_path();
        if let Some(parent) = std::path::Path::new(path).parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::write(path, content).await?;
        Ok(())
    }

    fn parse_routes(content: &str) -> Vec<Route> {
        let mut routes = Vec::new();
        let mut lines = content.lines().peekable();
        while let Some(line) = lines.next() {
            let line = line.trim();
            // A block starts with "domain.com {"
            if line.ends_with('{') && !line.starts_with('#') {
                let domain = line.trim_end_matches('{').trim().to_string();
                let mut port = 0u16;
                // Read until closing brace
                for inner in lines.by_ref() {
                    let inner = inner.trim();
                    if inner == "}" {
                        break;
                    }
                    if inner.starts_with("reverse_proxy") {
                        let target = inner.replace("reverse_proxy", "").trim().to_string();
                        // target is like "localhost:3000" or ":3000"
                        if let Some(p) = target.split(':').last() {
                            port = p.parse().unwrap_or(0);
                        }
                    }
                }
                if !domain.is_empty() && port > 0 {
                    routes.push(Route { domain, port, tls: true });
                }
            }
        }
        routes
    }
}

async fn run(args: &[&str]) -> Result<String> {
    let out = tokio::process::Command::new("caddy")
        .args(args)
        .output()
        .await?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    } else {
        anyhow::bail!("{}", String::from_utf8_lossy(&out.stderr).trim())
    }
}

#[async_trait]
impl GatewayManager for CaddyManager {
    async fn is_installed(&self) -> bool {
        tokio::process::Command::new("caddy")
            .arg("version")
            .output()
            .await
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    async fn version(&self) -> Option<String> {
        run(&["version"]).await.ok()
    }

    async fn install(&self) -> Result<String> {
        if crate::core::packages::which("apt-get") {
            let out = tokio::process::Command::new("apt-get")
                .args(["install", "-y", "caddy"])
                .output()
                .await?;
            if !out.status.success() {
                anyhow::bail!("{}", String::from_utf8_lossy(&out.stderr).trim());
            }
            return Ok(String::from_utf8_lossy(&out.stdout).to_string());
        }
        if crate::core::packages::which("dnf") {
            let out = tokio::process::Command::new("dnf")
                .args(["install", "-y", "caddy"])
                .output()
                .await?;
            if !out.status.success() {
                anyhow::bail!("{}", String::from_utf8_lossy(&out.stderr).trim());
            }
            return Ok(String::from_utf8_lossy(&out.stdout).to_string());
        }
        if crate::core::packages::which("brew") {
            let out = tokio::process::Command::new("brew")
                .args(["install", "caddy"])
                .output()
                .await?;
            if !out.status.success() {
                anyhow::bail!("{}", String::from_utf8_lossy(&out.stderr).trim());
            }
            return Ok(String::from_utf8_lossy(&out.stdout).to_string());
        }
        anyhow::bail!("No supported package manager found to install Caddy")
    }

    async fn install_streamed(
        &self,
        tx: tokio::sync::mpsc::UnboundedSender<String>,
    ) -> Result<String> {
        use crate::core::packages::run_cmd_streaming;
        if crate::core::packages::which("apt-get") {
            return run_cmd_streaming("apt-get", &["install", "-y", "caddy"], tx).await;
        }
        if crate::core::packages::which("dnf") {
            return run_cmd_streaming("dnf", &["install", "-y", "caddy"], tx).await;
        }
        if crate::core::packages::which("brew") {
            return run_cmd_streaming("brew", &["install", "caddy"], tx).await;
        }
        anyhow::bail!("No supported package manager found to install Caddy")
    }

    async fn list_routes(&self) -> Result<Vec<Route>> {
        let content = Self::read_caddyfile().await?;
        Ok(Self::parse_routes(&content))
    }

    async fn add_route(&self, route: Route) -> Result<()> {
        let mut content = Self::read_caddyfile().await?;
        let block = format!(
            "\n{} {{\n\treverse_proxy localhost:{}\n}}\n",
            route.domain, route.port
        );
        content.push_str(&block);
        Self::write_caddyfile(&content).await?;
        self.reload().await?;
        Ok(())
    }

    async fn remove_route(&self, domain: &str) -> Result<()> {
        let content = Self::read_caddyfile().await?;
        // Remove the block for this domain
        let mut result = String::new();
        let mut in_block = false;
        let mut depth = 0usize;
        for line in content.lines() {
            let trimmed = line.trim();
            if !in_block && trimmed.starts_with(domain) && trimmed.ends_with('{') {
                in_block = true;
                depth = 1;
                continue;
            }
            if in_block {
                depth += trimmed.chars().filter(|&c| c == '{').count();
                depth = depth.saturating_sub(trimmed.chars().filter(|&c| c == '}').count());
                if depth == 0 {
                    in_block = false;
                }
                continue;
            }
            result.push_str(line);
            result.push('\n');
        }
        Self::write_caddyfile(&result).await?;
        self.reload().await?;
        Ok(())
    }

    async fn reload(&self) -> Result<()> {
        // Prefer systemctl when Caddy runs as a managed service
        let systemctl_active = tokio::process::Command::new("systemctl")
            .args(["is-active", "--quiet", "caddy"])
            .output()
            .await
            .map(|o| o.status.success())
            .unwrap_or(false);

        if systemctl_active {
            let out = tokio::process::Command::new("systemctl")
                .args(["reload", "caddy"])
                .output()
                .await?;
            if out.status.success() {
                return Ok(());
            }
            anyhow::bail!("{}", String::from_utf8_lossy(&out.stderr).trim());
        }

        // Caddy not managed by systemd — check if admin API is reachable
        let api_up = tokio::process::Command::new("curl")
            .args(["-sf", "http://localhost:2019/config/"])
            .output()
            .await
            .map(|o| o.status.success())
            .unwrap_or(false);

        if api_up {
            let out = tokio::process::Command::new("caddy")
                .args(["reload", "--config", Self::caddyfile_path()])
                .output()
                .await?;
            if out.status.success() {
                return Ok(());
            }
            anyhow::bail!("{}", String::from_utf8_lossy(&out.stderr).trim());
        }

        // Caddy not running at all — start it
        let out = tokio::process::Command::new("caddy")
            .args(["start", "--config", Self::caddyfile_path()])
            .output()
            .await?;
        if out.status.success() {
            Ok(())
        } else {
            anyhow::bail!("{}", String::from_utf8_lossy(&out.stderr).trim())
        }
    }
}
