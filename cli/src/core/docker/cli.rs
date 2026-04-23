use anyhow::{Context, Result};
use async_trait::async_trait;
use tokio::process::Command;

use crate::core::{
    docker::DockerManager,
    models::{DockerComposeService, DockerContainer, DockerImage, ManagedDockerService},
};

pub struct DockerCliManager {
    /// "docker" or "podman", whichever was found first on PATH.
    bin: &'static str,
}

impl DockerCliManager {
    pub fn detect() -> Self {
        use crate::core::packages::which;
        let bin = if which("docker") { "docker" } else { "podman" };
        Self { bin }
    }

    fn get_field(v: &serde_json::Value, keys: &[&str]) -> String {
        for &key in keys {
            let val = &v[key];
            if let Some(s) = val.as_str() {
                return s.to_string();
            }
            if let Some(arr) = val.as_array() {
                let parts: Vec<String> = arr
                    .iter()
                    .filter_map(|x| {
                        if x.is_string() {
                            x.as_str().map(|s| s.to_string())
                        } else {
                            Some(x.to_string())
                        }
                    })
                    .collect();
                if !parts.is_empty() {
                    return parts.join(", ");
                }
            }
            if !val.is_null() && !val.is_object() {
                return val.to_string().trim_matches('"').to_string();
            }
        }
        String::new()
    }

    fn parse_containers(output: &str) -> Vec<DockerContainer> {
        output
            .lines()
            .filter_map(|line| {
                let v: serde_json::Value = serde_json::from_str(line).ok()?;
                Some(DockerContainer {
                    id: Self::get_field(&v, &["ID", "Id", "id"]),
                    name: Self::get_field(&v, &["Names", "names", "Name", "name"])
                        .trim_start_matches('/')
                        .to_string(),
                    image: Self::get_field(&v, &["Image", "image"]),
                    status: Self::get_field(&v, &["Status", "status", "State", "state"]),
                    ports: Self::get_field(&v, &["Ports", "ports"]),
                    created: Self::get_field(&v, &["CreatedAt", "created_at", "Created", "created"]),
                    cpu_pct: 0.0,
                    mem_usage: String::new(),
                })
            })
            .collect()
    }

    fn parse_images(output: &str) -> Vec<DockerImage> {
        output
            .lines()
            .filter_map(|line| {
                let v: serde_json::Value = serde_json::from_str(line).ok()?;
                Some(DockerImage {
                    id: Self::get_field(&v, &["ID", "Id", "id"]),
                    repository: Self::get_field(&v, &["Repository", "repository", "Repo", "repo"]),
                    tag: Self::get_field(&v, &["Tag", "tag"]),
                    size: Self::get_field(&v, &["Size", "size"]),
                    created: Self::get_field(&v, &["CreatedAt", "created_at", "Created", "created"]),
                })
            })
            .collect()
    }

    fn parse_compose(output: &str) -> Vec<DockerComposeService> {
        output
            .lines()
            .filter_map(|line| {
                let v: serde_json::Value = serde_json::from_str(line).ok()?;
                let name = Self::get_field(&v, &["Name", "name"]);
                if name.is_empty() { return None; }

                Some(DockerComposeService {
                    name,
                    status: Self::get_field(&v, &["Status", "status", "State", "state"]),
                    image: Self::get_field(&v, &["Image", "image"]),
                    ports: v["Publishers"]
                        .as_array()
                        .or_else(|| v["publishers"].as_array())
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|p| {
                                    let pub_port = p["PublishedPort"].as_u64()
                                        .or_else(|| p["published_port"].as_u64())?;
                                    let tgt_port = p["TargetPort"].as_u64()
                                        .or_else(|| p["target_port"].as_u64())?;
                                    if pub_port == 0 {
                                        None
                                    } else {
                                        Some(format!("{}:{}", pub_port, tgt_port))
                                    }
                                })
                                .collect::<Vec<_>>()
                                .join(", ")
                        })
                        .unwrap_or_else(|| Self::get_field(&v, &["Ports", "ports"])),
                })
            })
            .collect()
    }

}

#[async_trait]
impl DockerManager for DockerCliManager {
    async fn is_installed(&self) -> bool {
        Command::new(self.bin)
            .arg("version")
            .output()
            .await
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    async fn version(&self) -> Option<String> {
        let out = Command::new(self.bin)
            .args(["version", "--format", "{{.Client.Version}}"])
            .output()
            .await
            .ok()?;
        if out.status.success() {
            Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
        } else {
            None
        }
    }

    async fn list_containers(&self) -> Result<Vec<DockerContainer>> {
        let out = Command::new(self.bin)
            .args(["ps", "-a", "--format", "{{json .}}"])
            .output()
            .await
            .context("docker ps failed")?;
        Ok(Self::parse_containers(&String::from_utf8_lossy(&out.stdout)))
    }

    async fn list_images(&self) -> Result<Vec<DockerImage>> {
        let out = Command::new(self.bin)
            .args(["images", "--format", "{{json .}}"])
            .output()
            .await
            .context("docker images failed")?;
        Ok(Self::parse_images(&String::from_utf8_lossy(&out.stdout)))
    }

    async fn start_container(&self, id: &str) -> Result<()> {
        let out = Command::new(self.bin).args(["start", id]).output().await?;
        if out.status.success() { Ok(()) } else {
            anyhow::bail!("{}", String::from_utf8_lossy(&out.stderr))
        }
    }

    async fn stop_container(&self, id: &str) -> Result<()> {
        let out = Command::new(self.bin).args(["stop", id]).output().await?;
        if out.status.success() { Ok(()) } else {
            anyhow::bail!("{}", String::from_utf8_lossy(&out.stderr))
        }
    }

    async fn restart_container(&self, id: &str) -> Result<()> {
        let out = Command::new(self.bin).args(["restart", id]).output().await?;
        if out.status.success() { Ok(()) } else {
            anyhow::bail!("{}", String::from_utf8_lossy(&out.stderr))
        }
    }

    async fn remove_container(&self, id: &str) -> Result<()> {
        let out = Command::new(self.bin).args(["rm", "-f", id]).output().await?;
        if out.status.success() { Ok(()) } else {
            anyhow::bail!("{}", String::from_utf8_lossy(&out.stderr))
        }
    }

    async fn remove_image(&self, id: &str) -> Result<()> {
        let out = Command::new(self.bin).args(["rmi", id]).output().await?;
        if out.status.success() { Ok(()) } else {
            anyhow::bail!("{}", String::from_utf8_lossy(&out.stderr))
        }
    }

    async fn list_compose_services(&self, path: &str) -> Result<Vec<DockerComposeService>> {
        let out = Command::new(self.bin)
            .args(["compose", "-f", path, "ps", "--format", "json"])
            .output()
            .await
            .context("docker compose ps failed")?;
        let text = String::from_utf8_lossy(&out.stdout);
        // docker compose ps --format json outputs a JSON array OR JSON lines depending on version
        if let Ok(arr) = serde_json::from_str::<serde_json::Value>(&text) {
            let lines = arr
                .as_array()
                .map(|a| a.iter().map(|v| v.to_string()).collect::<Vec<_>>().join("\n"))
                .unwrap_or_default();
            return Ok(Self::parse_compose(&lines));
        }
        Ok(Self::parse_compose(&text))
    }

    async fn compose_up(&self, path: &str) -> Result<()> {
        let out = Command::new(self.bin)
            .args(["compose", "-f", path, "up", "-d"])
            .output()
            .await?;
        if out.status.success() { Ok(()) } else {
            anyhow::bail!("{}", String::from_utf8_lossy(&out.stderr))
        }
    }

    async fn compose_down(&self, path: &str) -> Result<()> {
        let out = Command::new(self.bin)
            .args(["compose", "-f", path, "down"])
            .output()
            .await?;
        if out.status.success() { Ok(()) } else {
            anyhow::bail!("{}", String::from_utf8_lossy(&out.stderr))
        }
    }

    async fn compose_restart(&self, path: &str) -> Result<()> {
        let out = Command::new(self.bin)
            .args(["compose", "-f", path, "restart"])
            .output()
            .await?;
        if out.status.success() { Ok(()) } else {
            anyhow::bail!("{}", String::from_utf8_lossy(&out.stderr))
        }
    }

    async fn list_managed_services(&self) -> Result<Vec<ManagedDockerService>> {
        let catalog = Self::managed_catalog();

        // Get all running container names in a single call
        let out = Command::new(self.bin)
            .args(["ps", "-a", "--format", "{{.Names}}\t{{.Status}}"])
            .output()
            .await
            .unwrap_or_else(|_| std::process::Output {
                status: std::process::ExitStatus::default(),
                stdout: vec![],
                stderr: vec![],
            });

        let container_statuses: std::collections::HashMap<String, String> = 
            String::from_utf8_lossy(&out.stdout)
                .lines()
                .filter_map(|line| {
                    let mut parts = line.splitn(2, '\t');
                    let name = parts.next()?.trim().to_string();
                    let status = parts.next().unwrap_or("").trim().to_string();
                    Some((name, status))
                })
                .collect();

        Ok(catalog.into_iter().map(|(name, container_name, image, ports, description)| {
            let status = container_statuses
                .get(container_name)
                .cloned()
                .unwrap_or_else(|| "not found".to_string());
            ManagedDockerService {
                name: name.to_string(),
                container_name: container_name.to_string(),
                image: image.to_string(),
                ports: ports.to_string(),
                status,
                description: description.to_string(),
            }
        }).collect())
    }

    async fn start_managed_service(&self, container_name: &str, image: &str, ports: &str) -> Result<()> {
        // If container already exists, just start it. Otherwise, run a new one.
        let exists_out = Command::new(self.bin)
            .args(["ps", "-a", "--filter", &format!("name=^{}$", container_name), "--format", "{{.Names}}"])
            .output()
            .await?;
        let exists = !String::from_utf8_lossy(&exists_out.stdout).trim().is_empty();

        if exists {
            let out = Command::new(self.bin).args(["start", container_name]).output().await?;
            if !out.status.success() {
                anyhow::bail!("{}", String::from_utf8_lossy(&out.stderr));
            }
        } else {
            // Build port args: "6379:6379" → ["-p", "6379:6379", ...]
            let mut args = vec!["run", "-d", "--name", container_name];
            let port_pairs: Vec<&str> = ports.split(',').map(|p| p.trim()).collect();
            for p in &port_pairs {
                args.push("-p");
                args.push(p);
            }
            args.push("--restart=unless-stopped");
            args.push(image);
            let out = Command::new(self.bin).args(&args).output().await?;
            if !out.status.success() {
                anyhow::bail!("{}", String::from_utf8_lossy(&out.stderr));
            }
        }
        Ok(())
    }

    async fn stop_managed_service(&self, container_name: &str) -> Result<()> {
        let out = Command::new(self.bin).args(["stop", container_name]).output().await?;
        if out.status.success() { Ok(()) } else {
            anyhow::bail!("{}", String::from_utf8_lossy(&out.stderr))
        }
    }

    async fn restart_managed_service(&self, container_name: &str) -> Result<()> {
        let out = Command::new(self.bin).args(["restart", container_name]).output().await?;
        if out.status.success() { Ok(()) } else {
            anyhow::bail!("{}", String::from_utf8_lossy(&out.stderr))
        }
    }
}

impl DockerCliManager {
    /// Catalog of predefined managed dev services:
    /// (display_name, container_name, image, ports, description)
    fn managed_catalog() -> Vec<(&'static str, &'static str, &'static str, &'static str, &'static str)> {
        vec![
            ("PostgreSQL",  "postlab-postgres",  "postgres:16-alpine",       "5432:5432", "Relational database (PostgreSQL 16)"),
            ("Redis",       "postlab-redis",     "redis:7-alpine",            "6379:6379", "In-memory key-value store & cache"),
            ("RabbitMQ",    "postlab-rabbitmq",  "rabbitmq:3-management",     "5672:5672,15672:15672", "Message broker with management UI"),
            ("MySQL",       "postlab-mysql",     "mysql:8",                   "3306:3306", "Relational database (MySQL 8)"),
            ("MongoDB",     "postlab-mongo",     "mongo:7",                   "27017:27017", "NoSQL document database"),
            ("Elasticsearch","postlab-elastic",  "elasticsearch:8.13.0",      "9200:9200,9300:9300", "Full-text search & analytics"),
            ("MinIO",       "postlab-minio",     "minio/minio",               "9000:9000,9001:9001", "S3-compatible object storage"),
            ("MailHog",     "postlab-mailhog",   "mailhog/mailhog",           "1025:1025,8025:8025", "Email testing (SMTP + web UI)"),
        ]
    }
}

