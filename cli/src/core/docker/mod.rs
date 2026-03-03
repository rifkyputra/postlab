use anyhow::Result;
use async_trait::async_trait;

use crate::core::models::{DockerComposeService, DockerContainer, DockerImage};

pub mod cli;
pub use cli::DockerCliManager;

#[async_trait]
pub trait DockerManager: Send + Sync {
    async fn is_installed(&self) -> bool;
    async fn version(&self) -> Option<String>;
    async fn list_containers(&self) -> Result<Vec<DockerContainer>>;
    async fn list_images(&self) -> Result<Vec<DockerImage>>;
    async fn start_container(&self, id: &str) -> Result<()>;
    async fn stop_container(&self, id: &str) -> Result<()>;
    async fn restart_container(&self, id: &str) -> Result<()>;
    async fn remove_container(&self, id: &str) -> Result<()>;
    async fn remove_image(&self, id: &str) -> Result<()>;
    async fn list_compose_services(&self, path: &str) -> Result<Vec<DockerComposeService>>;
    async fn compose_up(&self, path: &str) -> Result<()>;
    async fn compose_down(&self, path: &str) -> Result<()>;
    async fn compose_restart(&self, path: &str) -> Result<()>;
}
