use anyhow::Result;
use async_trait::async_trait;
use crate::core::models::Route;

pub mod caddy;
pub use caddy::CaddyManager;

#[async_trait]
pub trait GatewayManager: Send + Sync {
    async fn is_installed(&self) -> bool;
    async fn version(&self) -> Option<String>;
    async fn install(&self) -> Result<String>;
    async fn list_routes(&self) -> Result<Vec<Route>>;
    async fn add_route(&self, route: Route) -> Result<()>;
    async fn remove_route(&self, domain: &str) -> Result<()>;
    async fn reload(&self) -> Result<()>;

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
}
