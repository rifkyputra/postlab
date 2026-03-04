use anyhow::{Context, Result};
use async_trait::async_trait;
use tokio::process::Command;

use crate::core::{
    docker::DockerManager,
    models::{DockerComposeService, DockerContainer, DockerImage},
};

pub struct DockerCliManager;

impl DockerCliManager {
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
        Command::new("docker")
            .arg("version")
            .output()
            .await
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    async fn version(&self) -> Option<String> {
        let out = Command::new("docker")
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
        let out = Command::new("docker")
            .args(["ps", "-a", "--format", "{{json .}}"])
            .output()
            .await
            .context("docker ps failed")?;
        Ok(Self::parse_containers(&String::from_utf8_lossy(&out.stdout)))
    }

    async fn list_images(&self) -> Result<Vec<DockerImage>> {
        let out = Command::new("docker")
            .args(["images", "--format", "{{json .}}"])
            .output()
            .await
            .context("docker images failed")?;
        Ok(Self::parse_images(&String::from_utf8_lossy(&out.stdout)))
    }

    async fn start_container(&self, id: &str) -> Result<()> {
        let out = Command::new("docker").args(["start", id]).output().await?;
        if out.status.success() { Ok(()) } else {
            anyhow::bail!("{}", String::from_utf8_lossy(&out.stderr))
        }
    }

    async fn stop_container(&self, id: &str) -> Result<()> {
        let out = Command::new("docker").args(["stop", id]).output().await?;
        if out.status.success() { Ok(()) } else {
            anyhow::bail!("{}", String::from_utf8_lossy(&out.stderr))
        }
    }

    async fn restart_container(&self, id: &str) -> Result<()> {
        let out = Command::new("docker").args(["restart", id]).output().await?;
        if out.status.success() { Ok(()) } else {
            anyhow::bail!("{}", String::from_utf8_lossy(&out.stderr))
        }
    }

    async fn remove_container(&self, id: &str) -> Result<()> {
        let out = Command::new("docker").args(["rm", "-f", id]).output().await?;
        if out.status.success() { Ok(()) } else {
            anyhow::bail!("{}", String::from_utf8_lossy(&out.stderr))
        }
    }

    async fn remove_image(&self, id: &str) -> Result<()> {
        let out = Command::new("docker").args(["rmi", id]).output().await?;
        if out.status.success() { Ok(()) } else {
            anyhow::bail!("{}", String::from_utf8_lossy(&out.stderr))
        }
    }

    async fn list_compose_services(&self, path: &str) -> Result<Vec<DockerComposeService>> {
        let out = Command::new("docker")
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
        let out = Command::new("docker")
            .args(["compose", "-f", path, "up", "-d"])
            .output()
            .await?;
        if out.status.success() { Ok(()) } else {
            anyhow::bail!("{}", String::from_utf8_lossy(&out.stderr))
        }
    }

    async fn compose_down(&self, path: &str) -> Result<()> {
        let out = Command::new("docker")
            .args(["compose", "-f", path, "down"])
            .output()
            .await?;
        if out.status.success() { Ok(()) } else {
            anyhow::bail!("{}", String::from_utf8_lossy(&out.stderr))
        }
    }

    async fn compose_restart(&self, path: &str) -> Result<()> {
        let out = Command::new("docker")
            .args(["compose", "-f", path, "restart"])
            .output()
            .await?;
        if out.status.success() { Ok(()) } else {
            anyhow::bail!("{}", String::from_utf8_lossy(&out.stderr))
        }
    }
}
