use super::{run_cmd, PackageManager};
use crate::core::models::Package;
use anyhow::Result;
use async_trait::async_trait;

pub struct PacmanManager;

#[async_trait]
impl PackageManager for PacmanManager {
    fn name(&self) -> &'static str {
        "pacman"
    }

    async fn list_installed(&self) -> Result<Vec<Package>> {
        let out = run_cmd("pacman", &["-Q"]).await?;
        Ok(out
            .lines()
            .filter_map(|line| {
                let mut parts = line.splitn(2, ' ');
                let name = parts.next()?.to_string();
                let version = parts.next().unwrap_or("").to_string();
                Some(Package { name, version, description: String::new(), installed: true })
            })
            .collect())
    }

    async fn search(&self, query: &str) -> Result<Vec<Package>> {
        let out = run_cmd("pacman", &["-Ss", query]).await.unwrap_or_default();
        let mut packages = Vec::new();
        let mut lines = out.lines().peekable();
        while let Some(line) = lines.next() {
            // First line: "repo/name version [flags]"
            // Second line: "    description"
            if let Some((_, rest)) = line.split_once('/') {
                let mut parts = rest.splitn(2, ' ');
                let name = parts.next().unwrap_or("").to_string();
                let desc = lines.next().unwrap_or("").trim().to_string();
                packages.push(Package {
                    name,
                    version: String::new(),
                    description: desc,
                    installed: false,
                });
            }
        }
        Ok(packages)
    }

    async fn install(&self, name: &str) -> Result<String> {
        run_cmd("pacman", &["-S", "--noconfirm", name]).await
    }

    async fn remove(&self, name: &str) -> Result<String> {
        run_cmd("pacman", &["-R", "--noconfirm", name]).await
    }

    async fn upgrade_all(&self) -> Result<String> {
        run_cmd("pacman", &["-Syu", "--noconfirm"]).await
    }

    async fn update_cache(&self) -> Result<()> {
        run_cmd("pacman", &["-Sy"]).await.map(|_| ())
    }
}
