use crate::core::models::DeploymentType;
use anyhow::{Context, Result};
use std::path::Path;
use tokio::process::Command;

pub async fn start_deployment(deploy_type: &DeploymentType, dir: &Path) -> Result<()> {
    match deploy_type {
        DeploymentType::DockerCompose => {
            let status = Command::new("docker")
                .current_dir(dir)
                .args(["compose", "up", "-d"])
                .status()
                .await
                .context("Failed to execute docker compose up")?;

            if !status.success() {
                anyhow::bail!("docker compose up failed with status: {}", status);
            }
        }
        DeploymentType::WasmCloud => {
            let wadm_file = if dir.join("wadm.yaml").exists() {
                "wadm.yaml"
            } else if dir.join("wasmcloud.toml").exists() {
                "wasmcloud.toml" 
            } else {
                anyhow::bail!("No wadm.yaml found for WasmCloud deployment");
            };

            let status = Command::new("wash")
                .current_dir(dir)
                .args(["app", "deploy", wadm_file])
                .status()
                .await
                .context("Failed to execute wash app deploy")?;

            if !status.success() {
                anyhow::bail!("wash app deploy failed with status: {}", status);
            }
        }
        DeploymentType::Unknown => {
            anyhow::bail!("Cannot start deployment of unknown type");
        }
    }
    Ok(())
}

pub async fn stop_deployment(deploy_type: &DeploymentType, dir: &Path) -> Result<()> {
    match deploy_type {
        DeploymentType::DockerCompose => {
            let status = Command::new("docker")
                .current_dir(dir)
                .args(["compose", "down"])
                .status()
                .await
                .context("Failed to execute docker compose down")?;

            if !status.success() {
                anyhow::bail!("docker compose down failed with status: {}", status);
            }
        }
        DeploymentType::WasmCloud => {
            // Ideally we'd parse the app name from wadm.yaml, but for now we throw an error.
            anyhow::bail!("Teardown for WasmCloud is not fully implemented (needs app name parsing)");
        }
        DeploymentType::Unknown => {
            anyhow::bail!("Cannot stop deployment of unknown type");
        }
    }
    Ok(())
}
