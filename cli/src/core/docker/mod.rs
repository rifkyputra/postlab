use anyhow::Result;
use async_trait::async_trait;

use crate::core::models::{
    DockerComposeService, DockerContainer, DockerImage, ManagedDockerService,
};

pub mod cli;
pub use cli::DockerCliManager;

pub const OPENCLAW_CONTAINER_NAME: &str = "openclaw";
pub const OPENCLAW_IMAGE: &str = "alpine/openclaw:main";
pub const OPENCLAW_HOST_PORT: u16 = 8080;
pub const OPENCLAW_CONTAINER_PORT: u16 = 8080;

pub struct ContainerInspect {
    pub ports: Vec<String>,
    pub volumes: Vec<String>,
    pub env_vars: Vec<(String, String)>,
    pub docker_health: String,
}

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
    // ── Managed dev services ──────────────────────────────────────────────
    async fn list_managed_services(&self) -> Result<Vec<ManagedDockerService>>;
    async fn start_managed_service(
        &self,
        container_name: &str,
        image: &str,
        ports: &str,
    ) -> Result<()>;
    async fn stop_managed_service(&self, container_name: &str) -> Result<()>;
    async fn restart_managed_service(&self, container_name: &str) -> Result<()>;
    // ── Generic container utilities ───────────────────────────────────────
    async fn fetch_container_logs(&self, container: &str, tail: usize) -> Result<Vec<String>>;
    async fn pull_image(&self, image: &str) -> Result<()>;
    async fn inspect_container(&self, container: &str) -> Result<ContainerInspect>;
    async fn run_named_container(
        &self,
        name: &str,
        image: &str,
        ports: &[(&str, &str)],
        restart: &str,
    ) -> Result<()>;
}
