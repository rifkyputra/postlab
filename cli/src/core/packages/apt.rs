use super::{run_cmd, run_cmd_streaming, PackageManager};
use crate::core::models::Package;
use anyhow::Result;
use async_trait::async_trait;

pub struct AptManager;

#[async_trait]
impl PackageManager for AptManager {
    fn name(&self) -> &'static str {
        "apt"
    }

    async fn list_installed(&self) -> Result<Vec<Package>> {
        let out = run_cmd(
            "dpkg-query",
            &["-W", "-f=${Package}|${Version}|${binary:Summary}\n"],
        )
        .await?;
        Ok(out
            .lines()
            .filter_map(|line| {
                let parts: Vec<&str> = line.splitn(3, '|').collect();
                if parts.len() >= 2 {
                    Some(Package {
                        name: parts[0].to_string(),
                        version: parts[1].to_string(),
                        description: parts.get(2).unwrap_or(&"").trim().to_string(),
                        installed: true,
                    })
                } else {
                    None
                }
            })
            .collect())
    }

    async fn search(&self, query: &str) -> Result<Vec<Package>> {
        let out = run_cmd("apt-cache", &["search", "--names-only", query]).await?;
        Ok(out
            .lines()
            .filter_map(|line| {
                let (name, desc) = line.split_once(" - ")?;
                Some(Package {
                    name: name.trim().to_string(),
                    version: String::new(),
                    description: desc.trim().to_string(),
                    installed: false,
                })
            })
            .collect())
    }

    async fn install(&self, name: &str) -> Result<String> {
        run_cmd("apt-get", &["install", "-y", name]).await
    }

    async fn remove(&self, name: &str) -> Result<String> {
        run_cmd("apt-get", &["remove", "-y", name]).await
    }

    async fn install_streamed(
        &self,
        name: &str,
        tx: tokio::sync::mpsc::UnboundedSender<String>,
    ) -> Result<String> {
        run_cmd_streaming("apt-get", &["install", "-y", name], tx).await
    }

    async fn remove_streamed(
        &self,
        name: &str,
        tx: tokio::sync::mpsc::UnboundedSender<String>,
    ) -> Result<String> {
        run_cmd_streaming("apt-get", &["remove", "-y", name], tx).await
    }

    async fn check_packages(&self, names: &[&str]) -> Result<Vec<Package>> {
        if names.is_empty() {
            return Ok(vec![]);
        }
        let mut args = vec!["-W", "-f=${Package}|${Version}|${binary:Summary}\n"];
        args.extend_from_slice(names);
        // dpkg-query exits 1 when some names are not found but still prints the found ones,
        // so read stdout regardless of exit code.
        let output = tokio::process::Command::new("dpkg-query")
            .args(&args)
            .output()
            .await?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(stdout
            .lines()
            .filter_map(|line| {
                let parts: Vec<&str> = line.splitn(3, '|').collect();
                if parts.len() >= 2 && !parts[1].is_empty() {
                    Some(crate::core::models::Package {
                        name: parts[0].to_string(),
                        version: parts[1].to_string(),
                        description: parts.get(2).unwrap_or(&"").trim().to_string(),
                        installed: true,
                    })
                } else {
                    None
                }
            })
            .collect())
    }

    async fn upgrade_all(&self) -> Result<String> {
        run_cmd("apt-get", &["upgrade", "-y"]).await
    }

    async fn update_cache(&self) -> Result<()> {
        run_cmd("apt-get", &["update"]).await.map(|_| ())
    }
}
