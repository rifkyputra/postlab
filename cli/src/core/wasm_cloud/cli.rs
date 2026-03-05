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

    async fn install(&self) -> Result<String> {
        // macOS — correct tap is wasmcloud/wasmcloud
        if crate::core::packages::which("brew") {
            let mgr = crate::core::packages::BrewManager;
            use crate::core::packages::PackageManager;
            return mgr.install("wasmcloud/wasmcloud/wash").await;
        }

        // Debian/Ubuntu
        if crate::core::packages::which("apt-get") {
            // Official install script from wasmcloud docs
            let script = "curl -s https://raw.githubusercontent.com/wasmCloud/wash/main/install.sh | bash";
            let out = Command::new("sh")
                .args(["-c", script])
                .output()?;
            if !out.status.success() {
                anyhow::bail!("{}", String::from_utf8_lossy(&out.stderr).trim());
            }
            return Ok(String::from_utf8_lossy(&out.stdout).to_string());
        }

        // Fallback: cargo binstall (fast, no build required)
        if crate::core::packages::which("cargo-binstall") || crate::core::packages::which("cargo") {
            let out = Command::new("cargo")
                .args(["install", "wash-cli"])
                .output()?;
            if !out.status.success() {
                anyhow::bail!("{}", String::from_utf8_lossy(&out.stderr).trim());
            }
            return Ok(String::from_utf8_lossy(&out.stdout).to_string());
        }

        anyhow::bail!("Please install wash CLI manually: https://wasmcloud.com/docs/installation")
    }

    async fn install_streamed(
        &self,
        tx: tokio::sync::mpsc::UnboundedSender<String>,
    ) -> Result<String> {
        use crate::core::packages::run_cmd_streaming;

        // macOS — correct tap is wasmcloud/wasmcloud
        if crate::core::packages::which("brew") {
            use crate::core::packages::PackageManager;
            return crate::core::packages::BrewManager
                .install_streamed("wasmcloud/wasmcloud/wash", tx)
                .await;
        }

        // Debian/Ubuntu
        if crate::core::packages::which("apt-get") {
            let script = "curl -s https://raw.githubusercontent.com/wasmCloud/wash/main/install.sh | bash";
            return run_cmd_streaming("sh", &["-c", script], tx).await;
        }

        // Fallback: cargo install
        if crate::core::packages::which("cargo") {
            return run_cmd_streaming("cargo", &["install", "wash-cli"], tx).await;
        }

        anyhow::bail!("Please install wash CLI manually: https://wasmcloud.com/docs/installation")
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
