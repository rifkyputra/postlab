use crate::core::models::{DiskInfo, MemInfo, NetStats, OsInfo, SwapStatus};
use anyhow::Result;
use async_trait::async_trait;

pub mod sysinfo_impl;
pub use sysinfo_impl::SysinfoManager;

#[async_trait]
pub trait SystemInfo: Send + Sync {
    async fn info(&self) -> Result<OsInfo>;
    async fn cpu_pct(&self) -> Result<Vec<f32>>;
    async fn mem(&self) -> Result<MemInfo>;
    async fn disks(&self) -> Result<Vec<DiskInfo>>;
    async fn net(&self) -> Result<NetStats>;

    // ── Swap management ───────────────────────────────────────────────────────
    async fn swap_status(&self) -> Result<SwapStatus>;
    /// Create a swap file at `path` of `size_mb` MiB, activate it, and persist in /etc/fstab.
    async fn swap_create(&self, path: &str, size_mb: u64) -> Result<()>;
    /// Deactivate and delete a swap file, removing its /etc/fstab entry.
    async fn swap_delete(&self, path: &str) -> Result<()>;
    /// Activate an existing swap file/partition (`swapon`).
    async fn swap_enable(&self, path: &str) -> Result<()>;
    /// Deactivate a swap file/partition (`swapoff`).
    async fn swap_disable(&self, path: &str) -> Result<()>;
    /// Resize an existing swap file to `size_mb` MiB (deactivate → resize → reformat → reactivate).
    async fn swap_resize(&self, path: &str, size_mb: u64) -> Result<()>;
}
