use super::{run_cmd, PackageManager};
use crate::core::models::Package;
use anyhow::Result;
use async_trait::async_trait;

pub struct DnfManager {
    bin: &'static str, // "dnf" or "yum"
}

impl DnfManager {
    pub fn new() -> Self {
        let bin = if super::which("dnf") { "dnf" } else { "yum" };
        Self { bin }
    }
}

#[async_trait]
impl PackageManager for DnfManager {
    fn name(&self) -> &'static str {
        self.bin
    }

    async fn list_installed(&self) -> Result<Vec<Package>> {
        let out = run_cmd("rpm", &["-qa", "--queryformat", "%{NAME}|%{VERSION}|%{SUMMARY}\n"]).await?;
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
        let out = run_cmd(self.bin, &["search", query]).await.unwrap_or_default();
        Ok(out
            .lines()
            .filter_map(|line| {
                // dnf search output: "name.arch : description"
                let (name_arch, desc) = line.split_once(" : ")?;
                let name = name_arch.split('.').next()?.trim().to_string();
                Some(Package {
                    name,
                    version: String::new(),
                    description: desc.trim().to_string(),
                    installed: false,
                })
            })
            .collect())
    }

    async fn install(&self, name: &str) -> Result<String> {
        run_cmd(self.bin, &["install", "-y", name]).await
    }

    async fn remove(&self, name: &str) -> Result<String> {
        run_cmd(self.bin, &["remove", "-y", name]).await
    }

    async fn upgrade_all(&self) -> Result<String> {
        run_cmd(self.bin, &["upgrade", "-y"]).await
    }

    async fn update_cache(&self) -> Result<()> {
        run_cmd(self.bin, &["makecache"]).await.map(|_| ())
    }
}
