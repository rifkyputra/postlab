use anyhow::Result;
use async_trait::async_trait;
use crate::core::models::ProcessEntry;

pub mod sysinfo_impl;
pub use sysinfo_impl::SysinfoProcessManager;

#[async_trait]
pub trait ProcessManager: Send + Sync {
    async fn list(&self) -> Result<Vec<ProcessEntry>>;
    async fn kill(&self, pid: u32) -> Result<()>;
}
