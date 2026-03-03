use anyhow::Result;
use async_trait::async_trait;
use crate::core::models::{Tunnel, TunnelRoute};

pub mod cloudflare;
pub use cloudflare::CloudflareManager;

#[async_trait]
pub trait TunnelManager: Send + Sync {
    async fn is_installed(&self) -> bool;
    async fn version(&self) -> Option<String>;
    async fn install(&self) -> Result<String>;
    async fn login(&self) -> Result<()>;
    async fn list_tunnels(&self) -> Result<Vec<Tunnel>>;
    async fn create(&self, name: &str) -> Result<Tunnel>;
    async fn add_route(&self, route: TunnelRoute) -> Result<()>;
    /// Edit only the local config file — adds (or updates) a hostname entry without
    /// touching Cloudflare DNS. Useful when DNS was already configured elsewhere.
    async fn add_domain_to_config(&self, route: TunnelRoute) -> Result<()>;
    /// Write/load the LaunchAgent (macOS) or systemd unit (Linux) pointing at
    /// `~/.cloudflared/<tunnel_id>.yaml`.
    async fn install_service(&self, tunnel_id: &str) -> Result<()>;
    /// Read `~/.cloudflared/<tunnel_id>.yaml` content.
    async fn config_content(&self, tunnel_id: &str) -> Result<String>;
    /// Returns (active, enabled) from systemctl.
    async fn service_status(&self) -> Result<(bool, bool)>;
    async fn service_start(&self) -> Result<()>;
    async fn service_stop(&self) -> Result<()>;
    async fn service_restart(&self) -> Result<()>;
    /// Remove a hostname ingress entry from `~/.cloudflared/<tunnel_id>.yaml`.
    async fn remove_ingress(&self, tunnel_id: &str, hostname: &str) -> Result<()>;
    /// Copy config.yaml → /etc/cloudflared/config.yml then restart (no reinstall).
    async fn sync_config(&self) -> Result<()>;

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
