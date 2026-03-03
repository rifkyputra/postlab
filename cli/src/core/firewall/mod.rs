use anyhow::Result;
use async_trait::async_trait;

use crate::core::models::FirewallRule;

pub mod ufw;
pub use ufw::UfwManager;

/// Minimal firewall backend abstraction.
#[async_trait]
pub trait FirewallManager: Send + Sync {
    /// Returns (enabled, backend_name).
    async fn status(&self) -> Result<(bool, String)>;
    async fn list_rules(&self) -> Result<Vec<FirewallRule>>;
    /// Add a rule.  `action` is "allow" or "deny".
    async fn add_rule(&self, port: &str, proto: &str, from: &str, action: &str) -> Result<()>;
    /// Delete the rule with the given number (1-based, as reported by the backend).
    async fn delete_rule(&self, num: usize) -> Result<()>;
    /// Enable or disable the firewall.
    async fn set_enabled(&self, enabled: bool) -> Result<()>;
}

/// Stub used when no supported firewall tool is detected.
pub struct NoneManager;

#[async_trait]
impl FirewallManager for NoneManager {
    async fn status(&self) -> Result<(bool, String)> {
        Ok((false, "none".to_string()))
    }
    async fn list_rules(&self) -> Result<Vec<FirewallRule>> {
        Ok(Vec::new())
    }
    async fn add_rule(&self, _port: &str, _proto: &str, _from: &str, _action: &str) -> Result<()> {
        anyhow::bail!("No firewall manager available on this system")
    }
    async fn delete_rule(&self, _num: usize) -> Result<()> {
        anyhow::bail!("No firewall manager available on this system")
    }
    async fn set_enabled(&self, _enabled: bool) -> Result<()> {
        anyhow::bail!("No firewall manager available on this system")
    }
}
