use anyhow::Result;
use async_trait::async_trait;
use crate::core::models::SecurityFinding;

pub mod checks;
pub mod fail2ban;

pub use checks::DefaultSecurityAuditor;
pub use fail2ban::{DefaultFail2Ban, Fail2BanManager};

#[async_trait]
pub trait SecurityAuditor: Send + Sync {
    async fn scan(&self) -> Result<Vec<SecurityFinding>>;
    async fn apply(&self, id: &str) -> Result<String>;
}
