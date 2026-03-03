use anyhow::{Context, Result};
use async_trait::async_trait;
use tokio::process::Command;

use crate::core::{
    docker::DockerManager,
    models::{DockerComposeService, DockerContainer, DockerImage},
};

pub struct DockerCliManager;

impl DockerCliManager {
    fn parse_containers(output: &str) -> Vec<DockerContainer> {
        // `docker ps -a --format` outputs JSON lines — one per container
        output
            .lines()
            .filter_map(|line| {
                let v: serde_json::Value = serde_json::from_str(line).ok()?;
                Some(DockerContainer {
                    id: v["ID"].as_str().unwrap_or("").to_string(),
                    name: v["Names"].as_str().unwrap_or("").trim_start_matches('/').to_string(),
                    image: v["Image"].as_str().unwrap_or("").to_string(),
                    status: v["Status"].as_str().unwrap_or("").to_string(),
                    ports: v["Ports"].as_str().unwrap_or("").to_string(),
                    created: v["CreatedAt"].as_str().unwrap_or("").to_string(),
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
                    id: v["ID"].as_str().unwrap_or("").to_string(),
                    repository: v["Repository"].as_str().unwrap_or("").to_string(),
                    tag: v["Tag"].as_str().unwrap_or("").to_string(),
                    size: v["Size"].as_str().unwrap_or("").to_string(),
                    created: v["CreatedAt"].as_str().unwrap_or("").to_string(),
                })
            })
            .collect()
    }

    fn parse_compose(output: &str) -> Vec<DockerComposeService> {
        output
            .lines()
            .filter_map(|line| {
                let v: serde_json::Value = serde_json::from_str(line).ok()?;
                Some(DockerComposeService {
                    name: v["Name"].as_str().unwrap_or("").to_string(),
                    status: v["Status"].as_str().unwrap_or("").to_string(),
                    image: v["Image"].as_str().unwrap_or("").to_string(),
                    ports: v["Publishers"]
                        .as_array()
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|p| {
                                    let pub_port = p["PublishedPort"].as_u64()?;
                                    let tgt_port = p["TargetPort"].as_u64()?;
                                    if pub_port == 0 { None } else {
                                        Some(format!("{}:{}", pub_port, tgt_port))
                                    }
                                })
                                .collect::<Vec<_>>()
                                .join(", ")
                        })
                        .unwrap_or_default(),
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
