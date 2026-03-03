use super::{run_cmd, PackageManager};
use crate::core::models::Package;
use anyhow::Result;
use async_trait::async_trait;

pub struct BrewManager;

/// Run a brew command, transparently dropping root via `sudo -u $SUDO_USER` when needed.
/// Homebrew refuses to run as root; this wrapper handles the common `sudo postlab` case.
async fn brew(args: &[&str]) -> Result<String> {
    if nix::unistd::getuid().is_root() {
        let sudo_user = std::env::var("SUDO_USER").map_err(|_| {
            anyhow::anyhow!(
                "Homebrew cannot run as root and $SUDO_USER is unset.\n\
                 Run postlab as your normal user or install a Linux package manager."
            )
        })?;
        let mut full: Vec<&str> = vec!["-u", &sudo_user, "brew"];
        full.extend_from_slice(args);
        return run_cmd("sudo", &full).await;
    }
    run_cmd("brew", args).await
}

#[async_trait]
impl PackageManager for BrewManager {
    fn name(&self) -> &'static str {
        "brew"
    }

    async fn list_installed(&self) -> Result<Vec<Package>> {
        let out = brew(&["list", "--versions"]).await?;
        Ok(out
            .lines()
            .filter_map(|line| {
                let mut parts = line.splitn(2, ' ');
                let name = parts.next()?.to_string();
                let version = parts.next().unwrap_or("").split_whitespace().next().unwrap_or("").to_string();
                Some(Package { name, version, description: String::new(), installed: true })
            })
            .collect())
    }

    async fn search(&self, query: &str) -> Result<Vec<Package>> {
        let out = brew(&["search", query]).await.unwrap_or_default();
        Ok(out
            .lines()
            .filter(|l| !l.starts_with('=') && !l.is_empty())
            .map(|name| Package {
                name: name.trim().to_string(),
                version: String::new(),
                description: String::new(),
                installed: false,
            })
            .collect())
    }

    async fn install(&self, name: &str) -> Result<String> {
        brew(&["install", name]).await
    }

    async fn remove(&self, name: &str) -> Result<String> {
        brew(&["uninstall", name]).await
    }

    async fn upgrade_all(&self) -> Result<String> {
        brew(&["upgrade"]).await
    }

    async fn update_cache(&self) -> Result<()> {
        brew(&["update"]).await.map(|_| ())
    }
}
