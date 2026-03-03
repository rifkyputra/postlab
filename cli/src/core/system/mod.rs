use anyhow::Result;
use async_trait::async_trait;
use crate::core::models::{DiskInfo, MemInfo, NetStats, OsInfo};

pub mod sysinfo_impl;
pub use sysinfo_impl::SysinfoManager;

#[async_trait]
pub trait SystemInfo: Send + Sync {
    async fn info(&self) -> Result<OsInfo>;
    async fn cpu_pct(&self) -> Result<Vec<f32>>;
    async fn mem(&self) -> Result<MemInfo>;
    async fn disks(&self) -> Result<Vec<DiskInfo>>;
    async fn net(&self) -> Result<NetStats>;
}
