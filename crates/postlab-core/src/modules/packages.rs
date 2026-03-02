use anyhow::{bail, Result};
use crate::os_detect::PkgManager;
use crate::ssh::SshSession;

pub struct PackageManager<'a> {
    ssh: &'a SshSession,
    mgr: PkgManager,
}

impl<'a> PackageManager<'a> {
    pub fn new(ssh: &'a SshSession, mgr: PkgManager) -> Self {
        Self { ssh, mgr }
    }

    pub async fn update_cache(&self) -> Result<String> {
        let cmd = match self.mgr {
            PkgManager::Apt => "apt-get update -qq",
            PkgManager::Dnf => "dnf check-update -q || true",
        };
        self.ssh.exec(cmd).await
    }

    pub async fn install(&self, packages: &[&str]) -> Result<String> {
        if packages.is_empty() {
            bail!("no packages specified");
        }
        let pkgs = packages.join(" ");
        let cmd = match self.mgr {
            PkgManager::Apt => format!("DEBIAN_FRONTEND=noninteractive apt-get install -y {pkgs}"),
            PkgManager::Dnf => format!("dnf install -y {pkgs}"),
        };
        self.ssh.exec(&cmd).await
    }

    pub async fn upgrade_all(&self) -> Result<String> {
        let cmd = match self.mgr {
            PkgManager::Apt => "DEBIAN_FRONTEND=noninteractive apt-get upgrade -y",
            PkgManager::Dnf => "dnf upgrade -y",
        };
        self.ssh.exec(cmd).await
    }

    pub async fn remove(&self, packages: &[&str]) -> Result<String> {
        if packages.is_empty() {
            bail!("no packages specified");
        }
        let pkgs = packages.join(" ");
        let cmd = match self.mgr {
            PkgManager::Apt => format!("apt-get remove -y {pkgs}"),
            PkgManager::Dnf => format!("dnf remove -y {pkgs}"),
        };
        self.ssh.exec(&cmd).await
    }

    pub async fn list_installed(&self) -> Result<String> {
        let cmd = match self.mgr {
            PkgManager::Apt => "dpkg -l",
            PkgManager::Dnf => "rpm -qa",
        };
        self.ssh.exec(cmd).await
    }
}
