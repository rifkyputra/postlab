use super::SecurityAuditor;
use crate::core::{
    models::{SecurityFinding, Severity},
    platform::OsFamily,
    services::is_systemd_available,
};
use anyhow::Result;
use async_trait::async_trait;
use chrono::Local;
use std::path::Path;
use tokio::fs;

pub struct DefaultSecurityAuditor {
    os: OsFamily,
}

impl DefaultSecurityAuditor {
    pub fn new(os: OsFamily) -> Self {
        Self { os }
    }
}

// ── individual checks ──────────────────────────────────────────────────────

/// SSH root login — applicable on any Unix with sshd.
async fn check_ssh_root_login() -> Option<SecurityFinding> {
    let content = fs::read_to_string("/etc/ssh/sshd_config").await.ok()?;
    let enabled = content.lines().any(|l| {
        let l = l.trim();
        !l.starts_with('#')
            && l.to_lowercase().starts_with("permitrootlogin")
            && l.contains("yes")
    });
    if enabled {
        Some(SecurityFinding {
            id: "ssh_root_login".to_string(),
            title: "SSH root login enabled".to_string(),
            severity: Severity::Critical,
            description: "PermitRootLogin is set to yes in /etc/ssh/sshd_config".to_string(),
            file_path: Some("/etc/ssh/sshd_config".to_string()),
            fix_description: "Set PermitRootLogin no (a .bak backup will be created)".to_string(),
        })
    } else {
        None
    }
}

/// SSH password auth — applicable on any Unix with sshd.
async fn check_ssh_password_auth() -> Option<SecurityFinding> {
    let content = fs::read_to_string("/etc/ssh/sshd_config").await.ok()?;
    let enabled = content.lines().any(|l| {
        let l = l.trim();
        !l.starts_with('#')
            && l.to_lowercase().starts_with("passwordauthentication")
            && l.contains("yes")
    });
    if enabled {
        Some(SecurityFinding {
            id: "ssh_password_auth".to_string(),
            title: "SSH password authentication enabled".to_string(),
            severity: Severity::High,
            description: "PasswordAuthentication yes allows brute-force attacks".to_string(),
            file_path: Some("/etc/ssh/sshd_config".to_string()),
            fix_description: "Set PasswordAuthentication no".to_string(),
        })
    } else {
        None
    }
}

/// Firewall check — OS-specific: ufw (Debian), firewalld (Redhat/Arch), pf (macOS).
async fn check_firewall(os: OsFamily) -> Option<SecurityFinding> {
    match os {
        OsFamily::Debian => check_ufw().await,
        OsFamily::Redhat | OsFamily::Arch => check_firewalld().await,
        OsFamily::Macos => check_pf().await,
        OsFamily::Unknown => {
            // Try both common Linux firewalls as a best-effort fallback.
            if let Some(f) = check_ufw().await {
                return Some(f);
            }
            check_firewalld().await
        }
    }
}

async fn check_ufw() -> Option<SecurityFinding> {
    let out = tokio::process::Command::new("ufw")
        .arg("status")
        .output()
        .await
        .ok()?;
    let s = String::from_utf8_lossy(&out.stdout);
    if s.contains("inactive") {
        Some(SecurityFinding {
            id: "firewall_inactive".to_string(),
            title: "Firewall (ufw) is inactive".to_string(),
            severity: Severity::High,
            description: "ufw is installed but not enabled".to_string(),
            file_path: None,
            fix_description: "Run: ufw --force enable".to_string(),
        })
    } else {
        None
    }
}

async fn check_firewalld() -> Option<SecurityFinding> {
    if !is_systemd_available() {
        return None;
    }
    let out = tokio::process::Command::new("systemctl")
        .args(["is-active", "firewalld"])
        .output()
        .await
        .ok()?;
    if !out.status.success() {
        Some(SecurityFinding {
            id: "firewall_inactive".to_string(),
            title: "Firewall (firewalld) is inactive".to_string(),
            severity: Severity::High,
            description: "firewalld is not running".to_string(),
            file_path: None,
            fix_description: "Run: systemctl enable --now firewalld".to_string(),
        })
    } else {
        None
    }
}

async fn check_pf() -> Option<SecurityFinding> {
    let out = tokio::process::Command::new("pfctl")
        .arg("-s")
        .arg("info")
        .output()
        .await
        .ok()?;
    let s = String::from_utf8_lossy(&out.stdout);
    if s.contains("Disabled") {
        Some(SecurityFinding {
            id: "firewall_inactive".to_string(),
            title: "Firewall (pf) is disabled".to_string(),
            severity: Severity::High,
            description: "macOS packet filter (pf) is not active".to_string(),
            file_path: None,
            fix_description: "Enable the built-in firewall in System Settings → Network → Firewall".to_string(),
        })
    } else {
        None
    }
}

/// ASLR — Linux only (/proc/sys/kernel/randomize_va_space).
async fn check_aslr() -> Option<SecurityFinding> {
    let val = fs::read_to_string("/proc/sys/kernel/randomize_va_space")
        .await
        .ok()?;
    if val.trim() != "2" {
        Some(SecurityFinding {
            id: "aslr_disabled".to_string(),
            title: "ASLR not fully enabled".to_string(),
            severity: Severity::Medium,
            description: format!(
                "kernel.randomize_va_space = {} (should be 2)",
                val.trim()
            ),
            file_path: Some("/etc/sysctl.conf".to_string()),
            fix_description: "Set kernel.randomize_va_space=2 in /etc/sysctl.conf".to_string(),
        })
    } else {
        None
    }
}

/// Auto-updates check — OS-specific package.
async fn check_auto_updates(os: OsFamily) -> Option<SecurityFinding> {
    match os {
        OsFamily::Debian => check_unattended_upgrades().await,
        OsFamily::Redhat => check_dnf_automatic().await,
        // Arch / macOS / Unknown: no standard mechanism, skip.
        _ => None,
    }
}

async fn check_unattended_upgrades() -> Option<SecurityFinding> {
    let installed = tokio::process::Command::new("dpkg-query")
        .args(["-l", "unattended-upgrades"])
        .output()
        .await
        .map(|o| o.status.success())
        .unwrap_or(false);
    if !installed {
        Some(SecurityFinding {
            id: "no_auto_updates".to_string(),
            title: "Automatic security updates not configured".to_string(),
            severity: Severity::Low,
            description: "unattended-upgrades is not installed".to_string(),
            file_path: None,
            fix_description: "Run: apt install -y unattended-upgrades".to_string(),
        })
    } else {
        None
    }
}

async fn check_dnf_automatic() -> Option<SecurityFinding> {
    if !is_systemd_available() {
        return None;
    }
    let out = tokio::process::Command::new("systemctl")
        .args(["is-active", "dnf-automatic.timer"])
        .output()
        .await
        .ok()?;
    if !out.status.success() {
        Some(SecurityFinding {
            id: "no_auto_updates".to_string(),
            title: "Automatic security updates not configured".to_string(),
            severity: Severity::Low,
            description: "dnf-automatic.timer is not active".to_string(),
            file_path: None,
            fix_description:
                "Run: dnf install -y dnf-automatic && systemctl enable --now dnf-automatic.timer"
                    .to_string(),
        })
    } else {
        None
    }
}

// ── apply fixes ───────────────────────────────────────────────────────────

async fn backup_file(path: &str) -> Result<()> {
    if Path::new(path).exists() {
        let ts = Local::now().format("%Y%m%dT%H%M%S");
        let backup = format!("{}.bak.{}", path, ts);
        fs::copy(path, &backup).await?;
    }
    Ok(())
}

async fn sed_in_place(path: &str, from: &str, to: &str) -> Result<String> {
    backup_file(path).await?;
    let content = fs::read_to_string(path).await?;
    let updated = content.replace(from, to);
    fs::write(path, &updated).await?;
    Ok(format!("Updated {} — backed up before change", path))
}

// ── trait impl ────────────────────────────────────────────────────────────

#[async_trait]
impl SecurityAuditor for DefaultSecurityAuditor {
    async fn scan(&self) -> Result<Vec<SecurityFinding>> {
        let os = self.os;

        // SSH checks apply everywhere; ASLR is Linux-only; firewall and
        // auto-updates are dispatched per OS inside their helpers.
        let (ssh_root, ssh_pwd, firewall, auto_updates) = tokio::join!(
            check_ssh_root_login(),
            check_ssh_password_auth(),
            check_firewall(os),
            check_auto_updates(os),
        );

        // ASLR only makes sense on Linux.
        let aslr = if os.is_linux() {
            check_aslr().await
        } else {
            None
        };

        let mut findings: Vec<SecurityFinding> = [ssh_root, ssh_pwd, firewall, aslr, auto_updates]
            .into_iter()
            .flatten()
            .collect();

        findings.sort_by(|a, b| a.severity.cmp(&b.severity));
        Ok(findings)
    }

    async fn apply(&self, id: &str) -> Result<String> {
        match id {
            "ssh_root_login" => {
                sed_in_place(
                    "/etc/ssh/sshd_config",
                    "PermitRootLogin yes",
                    "PermitRootLogin no",
                )
                .await?;
                // sshd service name differs slightly across distros but
                // systemctl handles both "sshd" and "ssh" gracefully.
                let svc = if self.os == OsFamily::Debian { "ssh" } else { "sshd" };
                let _ = tokio::process::Command::new("systemctl")
                    .args(["restart", svc])
                    .output()
                    .await;
                Ok(format!("PermitRootLogin set to no, {} restarted", svc))
            }
            "ssh_password_auth" => {
                sed_in_place(
                    "/etc/ssh/sshd_config",
                    "PasswordAuthentication yes",
                    "PasswordAuthentication no",
                )
                .await?;
                let svc = if self.os == OsFamily::Debian { "ssh" } else { "sshd" };
                let _ = tokio::process::Command::new("systemctl")
                    .args(["restart", svc])
                    .output()
                    .await;
                Ok(format!("PasswordAuthentication set to no, {} restarted", svc))
            }
            "firewall_inactive" => match self.os {
                OsFamily::Debian => {
                    let out = tokio::process::Command::new("ufw")
                        .args(["--force", "enable"])
                        .output()
                        .await?;
                    Ok(String::from_utf8_lossy(&out.stdout).to_string())
                }
                OsFamily::Redhat | OsFamily::Arch => {
                    let out = tokio::process::Command::new("systemctl")
                        .args(["enable", "--now", "firewalld"])
                        .output()
                        .await?;
                    Ok(String::from_utf8_lossy(&out.stdout).to_string())
                }
                OsFamily::Macos => {
                    anyhow::bail!(
                        "Enable the macOS firewall in System Settings → Network → Firewall"
                    )
                }
                OsFamily::Unknown => {
                    anyhow::bail!("Cannot fix firewall: OS not recognised")
                }
            },
            "aslr_disabled" => {
                if !self.os.is_linux() {
                    anyhow::bail!("ASLR fix is only applicable on Linux");
                }
                backup_file("/etc/sysctl.conf").await?;
                let mut content =
                    fs::read_to_string("/etc/sysctl.conf").await.unwrap_or_default();
                if !content.contains("randomize_va_space") {
                    content.push_str("\nkernel.randomize_va_space=2\n");
                    fs::write("/etc/sysctl.conf", &content).await?;
                }
                let out = tokio::process::Command::new("sysctl")
                    .args(["-w", "kernel.randomize_va_space=2"])
                    .output()
                    .await?;
                Ok(String::from_utf8_lossy(&out.stdout).to_string())
            }
            "no_auto_updates" => match self.os {
                OsFamily::Debian => {
                    let out = tokio::process::Command::new("apt-get")
                        .args(["install", "-y", "unattended-upgrades"])
                        .output()
                        .await?;
                    Ok(String::from_utf8_lossy(&out.stdout).to_string())
                }
                OsFamily::Redhat => {
                    let out = tokio::process::Command::new("dnf")
                        .args(["install", "-y", "dnf-automatic"])
                        .output()
                        .await?;
                    let _ = tokio::process::Command::new("systemctl")
                        .args(["enable", "--now", "dnf-automatic.timer"])
                        .output()
                        .await;
                    Ok(String::from_utf8_lossy(&out.stdout).to_string())
                }
                _ => anyhow::bail!("Auto-update fix not supported on this OS"),
            },
            other => anyhow::bail!("Unknown finding id: {}", other),
        }
    }
}
