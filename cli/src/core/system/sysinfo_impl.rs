use super::SystemInfo;
use crate::core::models::{DiskInfo, MemInfo, NetStats, OsInfo, SwapEntry, SwapStatus};
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
        Ok(NetStats {
            rx_bytes: rx,
            tx_bytes: tx,
        })
    }

    async fn swap_status(&self) -> Result<SwapStatus> {
        let mut sys = self.sys.lock().await;
        sys.refresh_memory();
        let total = sys.total_swap();
        let used = sys.used_swap();
        drop(sys);
        let entries = parse_proc_swaps().await.unwrap_or_default();
        Ok(SwapStatus {
            total,
            used,
            free: total.saturating_sub(used),
            entries,
        })
    }

    async fn swap_create(&self, path: &str, size_mb: u64) -> Result<()> {
        let size_arg = format!("{}M", size_mb);
        run_cmd("fallocate", &["-l", &size_arg, path]).await?;
        run_cmd("chmod", &["600", path]).await?;
        run_cmd("mkswap", &[path]).await?;
        run_cmd("swapon", &[path]).await?;
        fstab_add(path).await
    }

    async fn swap_delete(&self, path: &str) -> Result<()> {
        // swapoff may fail if already inactive — ignore the error
        let _ = run_cmd("swapoff", &[path]).await;
        fstab_remove(path).await?;
        // Only delete regular files, not block devices
        if tokio::fs::metadata(path).await.map(|m| m.is_file()).unwrap_or(false) {
            tokio::fs::remove_file(path).await?;
        }
        Ok(())
    }

    async fn swap_enable(&self, path: &str) -> Result<()> {
        run_cmd("swapon", &[path]).await?;
        Ok(())
    }

    async fn swap_disable(&self, path: &str) -> Result<()> {
        run_cmd("swapoff", &[path]).await?;
        Ok(())
    }

    async fn swap_resize(&self, path: &str, size_mb: u64) -> Result<()> {
        run_cmd("swapoff", &[path]).await?;
        let size_arg = format!("{}M", size_mb);
        run_cmd("fallocate", &["-l", &size_arg, path]).await?;
        run_cmd("mkswap", &[path]).await?;
        run_cmd("swapon", &[path]).await?;
        Ok(())
    }
}

async fn run_cmd(program: &str, args: &[&str]) -> Result<String> {
    let output = tokio::process::Command::new(program)
        .args(args)
        .output()
        .await?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        anyhow::bail!("{}", String::from_utf8_lossy(&output.stderr).trim())
    }
}

async fn parse_proc_swaps() -> Result<Vec<SwapEntry>> {
    let content = tokio::fs::read_to_string("/proc/swaps").await?;
    let entries = content
        .lines()
        .skip(1) // skip header
        .filter_map(|line| {
            let cols: Vec<&str> = line.split_whitespace().collect();
            if cols.len() < 5 {
                return None;
            }
            Some(SwapEntry {
                path: cols[0].to_string(),
                kind: cols[1].to_string(),
                size_bytes: cols[2].parse::<u64>().ok()? * 1024,
                used_bytes: cols[3].parse::<u64>().ok()? * 1024,
                priority: cols[4].parse::<i32>().unwrap_or(-1),
            })
        })
        .collect();
    Ok(entries)
}

async fn fstab_add(path: &str) -> Result<()> {
    let content = tokio::fs::read_to_string("/etc/fstab").await.unwrap_or_default();
    if content.lines().any(|l| l.split_whitespace().next() == Some(path)) {
        return Ok(());
    }
    let entry = format!("\n{} none swap sw 0 0\n", path);
    use tokio::io::AsyncWriteExt;
    let mut file = tokio::fs::OpenOptions::new()
        .append(true)
        .open("/etc/fstab")
        .await?;
    file.write_all(entry.as_bytes()).await?;
    Ok(())
}

async fn fstab_remove(path: &str) -> Result<()> {
    let content = tokio::fs::read_to_string("/etc/fstab").await.unwrap_or_default();
    let filtered: String = content
        .lines()
        .filter(|l| l.split_whitespace().next() != Some(path))
        .map(|l| format!("{}\n", l))
        .collect();
    tokio::fs::write("/etc/fstab", filtered).await?;
    Ok(())
}
