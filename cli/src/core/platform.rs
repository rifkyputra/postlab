use std::sync::Arc;
use anyhow::Result;

use crate::core::{
    docker::{DockerCliManager, DockerManager},
    firewall::{FirewallManager, NoneManager, UfwManager},
    gateway::{CaddyManager, GatewayManager},
    packages::{
        AptManager, BrewManager, DnfManager, PackageManager, PacmanManager,
        which,
    },
    processes::{ProcessManager, SysinfoProcessManager},
    security::{DefaultFail2Ban, DefaultSecurityAuditor, Fail2BanManager, SecurityAuditor},
    ssh::{DefaultSshKeyManager, SshKeyManager},
    system::{SysinfoManager, SystemInfo},
    tunnel::{CloudflareManager, TunnelManager},
    wasm_cloud::{WasmCloudCliManager, WasmCloudManager},
};

/// Returns true when systemd is the init system (PID 1).
/// Checks `/run/systemd/private` — a directory created by systemd at boot that
/// is absent in Docker containers and other non-systemd environments.
pub fn is_systemd_available() -> bool {
    std::path::Path::new("/run/systemd/private").exists()
}

/// Coarse OS family used to gate security checks and fixes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OsFamily {
    Debian,  // Debian, Ubuntu, Raspbian …
    Redhat,  // Fedora, RHEL, CentOS, Rocky …
    Arch,    // Arch, Manjaro …
    Macos,
    Unknown,
}

impl OsFamily {
    pub fn detect() -> Self {
        // Cheapest signal: check for package managers in order.
        if which("apt-get") || which("apt") {
            return OsFamily::Debian;
        }
        if which("dnf") || which("yum") {
            return OsFamily::Redhat;
        }
        if which("pacman") {
            return OsFamily::Arch;
        }
        if cfg!(target_os = "macos") || which("brew") {
            return OsFamily::Macos;
        }
        OsFamily::Unknown
    }

    pub fn is_linux(self) -> bool {
        matches!(self, OsFamily::Debian | OsFamily::Redhat | OsFamily::Arch)
    }
}

pub struct Platform {
    pub os: OsFamily,
    pub system: Arc<dyn SystemInfo>,
    pub packages: Arc<dyn PackageManager>,
    pub processes: Arc<dyn ProcessManager>,
    pub security: Arc<dyn SecurityAuditor>,
    pub fail2ban: Arc<dyn Fail2BanManager>,
    pub gateway: Arc<dyn GatewayManager>,
    pub tunnel: Arc<dyn TunnelManager>,
    pub firewall: Arc<dyn FirewallManager>,
    pub docker: Arc<dyn DockerManager>,
    pub wasm_cloud: Arc<dyn WasmCloudManager>,
    pub ssh: Arc<dyn SshKeyManager>,
}

pub fn detect() -> Result<Platform> {
    let os = OsFamily::detect();
    let system = Arc::new(SysinfoManager::new());
    let processes = Arc::new(SysinfoProcessManager::new());
    let security = Arc::new(DefaultSecurityAuditor::new(os));
    let fail2ban = Arc::new(DefaultFail2Ban) as Arc<dyn Fail2BanManager>;
    let gateway = Arc::new(CaddyManager);
    let tunnel = Arc::new(CloudflareManager);
    let docker: Arc<dyn DockerManager> = Arc::new(DockerCliManager);
    let wasm_cloud: Arc<dyn WasmCloudManager> = Arc::new(WasmCloudCliManager);

    let packages: Arc<dyn PackageManager> = detect_package_manager()?;
    let firewall: Arc<dyn FirewallManager> = detect_firewall();
    let ssh = Arc::new(DefaultSshKeyManager);

    Ok(Platform {
        os,
        system,
        packages,
        processes,
        security,
        fail2ban,
        gateway,
        tunnel,
        firewall,
        docker,
        wasm_cloud,
        ssh,
    })
}

fn detect_firewall() -> Arc<dyn FirewallManager> {
    if which("ufw") {
        return Arc::new(UfwManager);
    }
    Arc::new(NoneManager)
}

fn detect_package_manager() -> Result<Arc<dyn PackageManager>> {
    // Linux — check in priority order
    if which("apt-get") || which("apt") {
        return Ok(Arc::new(AptManager));
    }
    if which("dnf") || which("yum") {
        return Ok(Arc::new(DnfManager::new()));
    }
    if which("pacman") {
        return Ok(Arc::new(PacmanManager));
    }
    // macOS
    if which("brew") {
        return Ok(Arc::new(BrewManager));
    }
    anyhow::bail!(
        "No supported package manager found (tried apt, dnf/yum, pacman, brew)"
    )
}
