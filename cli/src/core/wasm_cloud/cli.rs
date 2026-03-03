use anyhow::Result;
use async_trait::async_trait;
use std::process::Command;

use crate::core::models::{WasmCloudApp, WasmCloudComponent, WasmCloudHost, WasmCloudLink};
use crate::core::packages::which;
use super::WasmCloudManager;

pub struct WasmCloudCliManager;

#[async_trait]
impl WasmCloudManager for WasmCloudCliManager {
    async fn is_installed(&self) -> bool {
        which("wash")
    }

    async fn version(&self) -> Option<String> {
        let output = Command::new("wash")
            .arg("--version")
            .output()
            .ok()?;
        
        if output.status.success() {
            let s = String::from_utf8_lossy(&output.stdout).trim().to_string();
            // wash 0.26.0 -> 0.26.0
            Some(s.replace("wash ", ""))
        } else {
            None
        }
    }

    async fn list_hosts(&self) -> Result<Vec<WasmCloudHost>> {
        // wash get inventory --output json
        // For now, returning empty to unblock TUI
        Ok(vec![])
    }

    async fn start_host(&self) -> Result<()> {
        // systemctl start wasmcloud
        Ok(())
    }

    async fn stop_host(&self) -> Result<()> {
        // systemctl stop wasmcloud
        Ok(())
    }

    async fn list_components(&self) -> Result<Vec<WasmCloudComponent>> {
        Ok(vec![])
    }

    async fn list_links(&self) -> Result<Vec<WasmCloudLink>> {
        Ok(vec![])
    }

    async fn list_apps(&self) -> Result<Vec<WasmCloudApp>> {
        Ok(vec![])
    }

    async fn deploy_app(&self, _manifest_path: &str) -> Result<()> {
        Ok(())
    }

    async fn undeploy_app(&self, _name: &str) -> Result<()> {
        Ok(())
    }

    async fn inspect_component(&self, wasm_path: &str) -> Result<String> {
        let output = Command::new("wash")
            .arg("inspect")
            .arg(wasm_path)
            .output()?;
        
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}
