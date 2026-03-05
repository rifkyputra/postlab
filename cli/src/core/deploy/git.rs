use anyhow::{Context, Result};
use std::path::Path;
use tokio::process::Command;

pub async fn clone_repo(url: &str, target_dir: &Path) -> Result<()> {
    let status = Command::new("git")
        .args(["clone", url, target_dir.to_str().unwrap()])
        .status()
        .await
        .context("Failed to execute git clone")?;

    if !status.success() {
        anyhow::bail!("git clone failed with status: {}", status);
    }
    Ok(())
}

pub async fn pull_repo(target_dir: &Path) -> Result<()> {
    let status = Command::new("git")
        .current_dir(target_dir)
        .arg("pull")
        .status()
        .await
        .context("Failed to execute git pull")?;

    if !status.success() {
        anyhow::bail!("git pull failed with status: {}", status);
    }
    Ok(())
}
