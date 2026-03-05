use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Returns true when systemd is the init system (PID 1).
pub fn is_systemd_available() -> bool {
    std::path::Path::new("/run/systemd/private").exists()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceUnit {
    pub name: String,
    pub description: String,
    pub load_state: String,   // loaded, not-found, bad-setting, etc.
    pub active_state: String, // active, inactive, failed, etc.
    pub sub_state: String,    // running, exited, dead, etc.
}

#[async_trait]
pub trait ServiceManager: Send + Sync {
    async fn list_services(&self) -> Result<Vec<ServiceUnit>>;
    async fn start(&self, name: &str) -> Result<()>;
    async fn stop(&self, name: &str) -> Result<()>;
    async fn restart(&self, name: &str) -> Result<()>;
    async fn enable(&self, name: &str) -> Result<()>;
    async fn disable(&self, name: &str) -> Result<()>;
}

pub struct SystemdServiceManager;

#[async_trait]
impl ServiceManager for SystemdServiceManager {
    async fn list_services(&self) -> Result<Vec<ServiceUnit>> {
        // We use --all and --type=service to get everything.
        // Format: UNIT LOAD ACTIVE SUB DESCRIPTION
        let output = tokio::process::Command::new("systemctl")
            .args(["list-units", "--type=service", "--all", "--no-legend", "--no-pager"])
            .output()
            .await?;

        if !output.status.success() {
            anyhow::bail!("systemctl failed: {}", String::from_utf8_lossy(&output.stderr));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut services = Vec::new();

        for line in stdout.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 5 {
                continue;
            }

            // parts[0] is unit name
            // parts[1] is load
            // parts[2] is active
            // parts[3] is sub
            // parts[4..] is description
            let description = parts[4..].join(" ");
            
            services.push(ServiceUnit {
                name: parts[0].to_string(),
                description,
                load_state: parts[1].to_string(),
                active_state: parts[2].to_string(),
                sub_state: parts[3].to_string(),
            });
        }

        Ok(services)
    }

    async fn start(&self, name: &str) -> Result<()> {
        run_systemctl(vec!["start", name]).await
    }

    async fn stop(&self, name: &str) -> Result<()> {
        run_systemctl(vec!["stop", name]).await
    }

    async fn restart(&self, name: &str) -> Result<()> {
        run_systemctl(vec!["restart", name]).await
    }

    async fn enable(&self, name: &str) -> Result<()> {
        run_systemctl(vec!["enable", name]).await
    }

    async fn disable(&self, name: &str) -> Result<()> {
        run_systemctl(vec!["disable", name]).await
    }
}

async fn run_systemctl(args: Vec<&str>) -> Result<()> {
    let output = tokio::process::Command::new("systemctl")
        .args(args)
        .output()
        .await?;

    if !output.status.success() {
        anyhow::bail!("systemctl failed: {}", String::from_utf8_lossy(&output.stderr));
    }

    Ok(())
}

pub struct MacosServiceManager;

#[async_trait]
impl ServiceManager for MacosServiceManager {
    async fn list_services(&self) -> Result<Vec<ServiceUnit>> {
        // macOS 'launchctl list' is very different.
        // We'll provide a minimal implementation or return empty for now.
        Ok(vec![])
    }

    async fn start(&self, _name: &str) -> Result<()> { anyhow::bail!("Not implemented for macOS") }
    async fn stop(&self, _name: &str) -> Result<()> { anyhow::bail!("Not implemented for macOS") }
    async fn restart(&self, _name: &str) -> Result<()> { anyhow::bail!("Not implemented for macOS") }
    async fn enable(&self, _name: &str) -> Result<()> { anyhow::bail!("Not implemented for macOS") }
    async fn disable(&self, _name: &str) -> Result<()> { anyhow::bail!("Not implemented for macOS") }
}
