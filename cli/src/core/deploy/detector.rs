use crate::core::models::DeploymentType;
use std::path::Path;

pub fn detect_deployment_type(dir: &Path) -> DeploymentType {
    if dir.join("docker-compose.yml").exists() || dir.join("compose.yaml").exists() {
        return DeploymentType::DockerCompose;
    }
    
    if dir.join("wadm.yaml").exists() || dir.join("wasmcloud.toml").exists() {
        return DeploymentType::WasmCloud;
    }

    DeploymentType::Unknown
}
