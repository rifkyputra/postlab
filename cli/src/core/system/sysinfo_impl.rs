use super::SystemInfo;
use crate::core::models::{DiskInfo, MemInfo, NetStats, OsInfo};
use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;
use sysinfo::{Disks, Networks, System};
use tokio::sync::Mutex;

pub struct SysinfoManager {
    sys: Arc<Mutex<System>>,
}

impl SysinfoManager {
    pub fn new() -> Self {
        let mut sys = System::new_all();
        sys.refresh_all();
        Self {
            sys: Arc::new(Mutex::new(sys)),
        }
    }
}

#[async_trait]
impl SystemInfo for SysinfoManager {
    async fn info(&self) -> Result<OsInfo> {
        let sys = self.sys.lock().await;
        Ok(OsInfo {
            hostname: System::host_name().unwrap_or_default(),
            distro: System::long_os_version().unwrap_or_default(),
            kernel_version: System::kernel_version().unwrap_or_default(),
            arch: std::env::consts::ARCH.to_string(),
            cpu_count: sys.cpus().len(),
            total_memory: sys.total_memory(),
            used_memory: sys.used_memory(),
            uptime_secs: System::uptime(),
        })
    }

    async fn cpu_pct(&self) -> Result<Vec<f32>> {
        let mut sys = self.sys.lock().await;
        sys.refresh_all();
        Ok(sys.cpus().iter().map(|c| c.cpu_usage()).collect())
    }

    async fn mem(&self) -> Result<MemInfo> {
        let mut sys = self.sys.lock().await;
        sys.refresh_memory();
        Ok(MemInfo {
            total: sys.total_memory(),
            used: sys.used_memory(),
            available: sys.available_memory(),
        })
    }

    async fn disks(&self) -> Result<Vec<DiskInfo>> {
        let disks = Disks::new_with_refreshed_list();
        Ok(disks
            .iter()
            .map(|d| DiskInfo {
                mount: d.mount_point().to_string_lossy().to_string(),
                total: d.total_space(),
                used: d.total_space().saturating_sub(d.available_space()),
                fs_type: d.file_system().to_string_lossy().to_string(),
            })
            .collect())
    }

    async fn net(&self) -> Result<NetStats> {
        let networks = Networks::new_with_refreshed_list();
        let rx: u64 = networks.iter().map(|(_, n)| n.received()).sum();
        let tx: u64 = networks.iter().map(|(_, n)| n.transmitted()).sum();
        Ok(NetStats { rx_bytes: rx, tx_bytes: tx })
    }
}
