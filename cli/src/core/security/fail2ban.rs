use anyhow::Result;
use async_trait::async_trait;
use tokio::process::Command;

use crate::core::models::JailedIp;

// ── Trait ─────────────────────────────────────────────────────────────────

#[async_trait]
pub trait Fail2BanManager: Send + Sync {
    /// Returns true if fail2ban-client is installed and reachable.
    async fn is_installed(&self) -> bool;
    /// List all currently-banned IPs across all jails.
    async fn list_jailed(&self) -> Result<Vec<JailedIp>>;
    /// Unban an IP from a specific jail (Forgive).
    async fn unban(&self, jail: &str, ip: &str) -> Result<()>;
    /// Permanently block an IP via firewall + keep it banned (Banish).
    async fn banish(&self, jail: &str, ip: &str) -> Result<()>;
}

// ── Implementation ────────────────────────────────────────────────────────

pub struct DefaultFail2Ban;

#[async_trait]
impl Fail2BanManager for DefaultFail2Ban {
    async fn is_installed(&self) -> bool {
        crate::core::packages::which("fail2ban-client")
    }

    async fn list_jailed(&self) -> Result<Vec<JailedIp>> {
        // Step 1: get list of jail names
        let jails = get_jail_names().await?;

        // Step 2: for each jail, get the banned IPs
        let mut result = Vec::new();
        for jail in &jails {
            if let Ok(entries) = get_jailed_for_jail(jail).await {
                result.extend(entries);
            }
        }
        Ok(result)
    }

    async fn unban(&self, jail: &str, ip: &str) -> Result<()> {
        run_f2b(&["set", jail, "unbanip", ip]).await?;
        Ok(())
    }

    async fn banish(&self, jail: &str, ip: &str) -> Result<()> {
        // The IP is already banned in fail2ban (still in the jailed list).
        // Add a permanent firewall DROP rule so it stays blocked even after
        // fail2ban's bantime expires.

        // Try UFW first (most common on Debian/Ubuntu homelab setups).
        let ufw_ok = Command::new("ufw")
            .args(["deny", "from", ip, "to", "any"])
            .output()
            .await
            .map(|o| o.status.success())
            .unwrap_or(false);

        // Fallback: direct iptables INSERT at position 1.
        if !ufw_ok {
            let _ = Command::new("iptables")
                .args(["-I", "INPUT", "1", "-s", ip, "-j", "DROP"])
                .output()
                .await;
        }

        // Optionally also re-ban in fail2ban (in case it was about to expire
        // right as the user pressed Banish).
        let _ = run_f2b(&["set", jail, "banip", ip]).await;

        Ok(())
    }
}

// ── Helpers ────────────────────────────────────────────────────────────────

/// Run fail2ban-client with the given args, trying without sudo first,
/// then with sudo if the process exits non-zero.
async fn run_f2b(args: &[&str]) -> Result<String> {
    let out = Command::new("fail2ban-client")
        .args(args)
        .output()
        .await?;

    if out.status.success() {
        return Ok(String::from_utf8_lossy(&out.stdout).into_owned());
    }

    // Retry with sudo
    let out = Command::new("sudo")
        .arg("-n")  // non-interactive: fail if password required
        .arg("fail2ban-client")
        .args(args)
        .output()
        .await?;

    if out.status.success() {
        return Ok(String::from_utf8_lossy(&out.stdout).into_owned());
    }

    anyhow::bail!(
        "fail2ban-client {}: {}",
        args.join(" "),
        String::from_utf8_lossy(&out.stderr).trim()
    )
}

/// Parse `fail2ban-client status` output to extract jail names.
///
/// Example output:
/// ```text
/// Status
/// |- Number of jail:      2
/// `- Jail list:   sshd, nginx-http-auth
/// ```
async fn get_jail_names() -> Result<Vec<String>> {
    let output = run_f2b(&["status"]).await?;
    let jail_line = output
        .lines()
        .find(|l| l.contains("Jail list"))
        .ok_or_else(|| anyhow::anyhow!("Could not find jail list in fail2ban-client status output"))?;

    let names: Vec<String> = jail_line
        .splitn(2, ':')
        .nth(1)
        .unwrap_or("")
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    Ok(names)
}

/// Parse `fail2ban-client status <jail>` and return one `JailedIp` per banned IP.
///
/// Example output:
/// ```text
/// Status for the jail: sshd
/// |- Filter
/// |  |- Currently failed: 3
/// |  |- Total failed:     47
/// |  `- File list:        /var/log/auth.log
/// `- Actions
///    |- Currently banned: 2
///    |- Total banned:     23
///    `- Banned IP list:   1.2.3.4 5.6.7.8
/// ```
async fn get_jailed_for_jail(jail: &str) -> Result<Vec<JailedIp>> {
    let output = run_f2b(&["status", jail]).await?;

    let total_failures: u32 = output
        .lines()
        .find(|l| l.contains("Total failed"))
        .and_then(|l| l.splitn(2, ':').nth(1))
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0);

    let ip_line = output
        .lines()
        .find(|l| l.contains("Banned IP list"))
        .and_then(|l| l.splitn(2, ':').nth(1))
        .unwrap_or("");

    let ips: Vec<JailedIp> = ip_line
        .split_whitespace()
        .map(|ip| JailedIp {
            ip: ip.to_string(),
            jail: jail.to_string(),
            total_failures,
        })
        .collect();

    Ok(ips)
}
