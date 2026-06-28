use super::{run_cmd, PackageManager};
use crate::core::models::{Package, UpgradablePackage};
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
        let out = run_cmd(
            "rpm",
            &["-qa", "--queryformat", "%{NAME}|%{VERSION}|%{SUMMARY}\n"],
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
        let out = run_cmd(self.bin, &["search", query])
            .await
            .unwrap_or_default();
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

    async fn clean_cache(&self) -> Result<String> {
        run_cmd(self.bin, &["clean", "all"]).await
    }

    async fn list_upgradable(&self) -> Result<Vec<UpgradablePackage>> {
        // `dnf check-update` exits 100 when updates are available, 0 when none,
        // and anything else on a real error — so we can't route it through run_cmd.
        let output = tokio::process::Command::new(self.bin)
            .arg("check-update")
            .output()
            .await?;
        let code = output.status.code().unwrap_or(-1);
        if code != 0 && code != 100 {
            return Ok(Vec::new());
        }
        let out = String::from_utf8_lossy(&output.stdout);
        Ok(out
            .lines()
            .filter_map(|line| {
                // "openssh-server.x86_64    9.6p1-3.el9_5    rhel-9-appstream"
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() < 3 {
                    return None;
                }
                let name_arch = parts[0];
                // Skip header/status lines ("Last metadata…", "Obsoleting Packages").
                if !name_arch.contains('.') {
                    return None;
                }
                let name = name_arch.split('.').next()?.to_string();
                let new_ver = parts[1].to_string();
                let repo = parts[2].to_string();
                Some(UpgradablePackage {
                    name,
                    current_version: String::new(),
                    new_version: new_ver,
                    repository: repo.clone(),
                    is_security: repo.contains("security"),
                })
            })
            .collect())
    }
}
