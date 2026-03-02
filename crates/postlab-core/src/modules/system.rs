use anyhow::Result;
use serde::{Deserialize, Serialize};
use crate::ssh::SshSession;

#[derive(Debug, Serialize, Deserialize)]
pub struct SystemStatus {
    pub uptime: String,
    pub load: String,
    pub memory: String,
    pub disk: String,
}

pub async fn get_status(ssh: &SshSession) -> Result<SystemStatus> {
    let uptime = ssh.exec("uptime -p").await?;
    let load = ssh.exec("cat /proc/loadavg").await?;
    let memory = ssh.exec("free -h | grep Mem").await?;
    let disk = ssh.exec("df -h / | tail -1").await?;

    Ok(SystemStatus {
        uptime: uptime.trim().to_string(),
        load: load.trim().to_string(),
        memory: memory.trim().to_string(),
        disk: disk.trim().to_string(),
    })
}
