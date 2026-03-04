use anyhow::Result;
use async_trait::async_trait;

use crate::core::models::{WasmCloudApp, WasmCloudComponent, WasmCloudHost, WasmCloudLink};

pub mod cli;
pub use cli::WasmCloudCliManager;

#[async_trait]
pub trait WasmCloudManager: Send + Sync {
    async fn is_installed(&self) -> bool;
    async fn version(&self) -> Option<String>;
    async fn install(&self) -> Result<String>;
    
    /// Install with line-by-line progress forwarded to `tx`. Default falls back to `install()`.
    async fn install_streamed(
        &self,
        tx: tokio::sync::mpsc::UnboundedSender<String>,
    ) -> Result<String> {
        match self.install().await {
            Ok(out) => {
                for line in out.lines() {
                    let _ = tx.send(line.to_string());
                }
                Ok(out)
            }
            Err(e) => {
                let msg = e.to_string();
                for line in msg.lines() {
                    let _ = tx.send(line.to_string());
                }
                Err(e)
            }
        }
    }

    // Host management
    async fn list_hosts(&self) -> Result<Vec<WasmCloudHost>>;
    async fn start_host(&self) -> Result<()>;
    async fn stop_host(&self) -> Result<()>;
    
    // Inventory
    async fn list_components(&self) -> Result<Vec<WasmCloudComponent>>;
    async fn list_links(&self) -> Result<Vec<WasmCloudLink>>;
    
    // Applications (WADM)
    async fn list_apps(&self) -> Result<Vec<WasmCloudApp>>;
    async fn deploy_app(&self, manifest_path: &str) -> Result<()>;
    async fn undeploy_app(&self, name: &str) -> Result<()>;
    
    // Inspection
    async fn inspect_component(&self, wasm_path: &str) -> Result<String>;
}
