use super::ProcessManager;
use crate::core::models::ProcessEntry;
use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;
use sysinfo::System;
use tokio::sync::Mutex;

pub struct SysinfoProcessManager {
    sys: Arc<Mutex<System>>,
}

impl SysinfoProcessManager {
    pub fn new() -> Self {
        let mut sys = System::new_all();
        sys.refresh_all();
        Self {
            sys: Arc::new(Mutex::new(sys)),
        }
    }
}

#[async_trait]
impl ProcessManager for SysinfoProcessManager {
    async fn list(&self) -> Result<Vec<ProcessEntry>> {
        let mut sys = self.sys.lock().await;
        sys.refresh_all();
        let mut entries: Vec<ProcessEntry> = sys
            .processes()
            .values()
            .map(|p| ProcessEntry {
                pid: p.pid().as_u32(),
                name: p.name().to_string(),
                cpu_pct: p.cpu_usage(),
                mem_bytes: p.memory(),
                user: p
                    .user_id()
                    .map(|uid| uid.to_string())
                    .unwrap_or_default(),
                status: p.status().to_string(),
            })
            .collect();
        entries.sort_by(|a, b| b.cpu_pct.partial_cmp(&a.cpu_pct).unwrap_or(std::cmp::Ordering::Equal));
        Ok(entries)
    }

    async fn kill(&self, pid: u32) -> Result<()> {
        let output = tokio::process::Command::new("kill")
            .args(["-15", &pid.to_string()])
            .output()
            .await?;
        if output.status.success() {
            Ok(())
        } else {
            anyhow::bail!(
                "Failed to kill process {}: {}",
                pid,
                String::from_utf8_lossy(&output.stderr)
            )
        }
    }
}
