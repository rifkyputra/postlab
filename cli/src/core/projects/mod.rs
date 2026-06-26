use anyhow::Result;
use std::cmp::Reverse;
use std::path::PathBuf;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::mpsc::UnboundedSender;

#[derive(Debug, Clone)]
pub struct ProjectEntry {
    pub name: String,
    pub path: String,
    pub modified: Option<u64>,
}

pub struct ProjectsManager;

pub fn expand_home(path: &str) -> String {
    if path.starts_with('~') {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
        path.replacen('~', &home, 1)
    } else {
        path.to_string()
    }
}

impl ProjectsManager {
    pub async fn list(&self, dir: &str) -> Result<Vec<ProjectEntry>> {
        let expanded = expand_home(dir);
        let path = PathBuf::from(&expanded);

        if !path.exists() {
            return Ok(Vec::new());
        }

        let mut entries = Vec::new();
        let mut read_dir = tokio::fs::read_dir(&path).await?;
        while let Some(entry) = read_dir.next_entry().await? {
            let metadata = entry.metadata().await?;
            if !metadata.is_dir() {
                continue;
            }
            let modified = metadata
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs());
            entries.push(ProjectEntry {
                name: entry.file_name().to_string_lossy().to_string(),
                path: entry.path().to_string_lossy().to_string(),
                modified,
            });
        }
        entries.sort_by_key(|e| Reverse(e.modified));
        Ok(entries)
    }

    pub async fn clone_repo(
        &self,
        url: &str,
        dir: &str,
        tx: UnboundedSender<String>,
    ) -> Result<String> {
        let expanded = expand_home(dir);
        tokio::fs::create_dir_all(&expanded).await?;

        let full_url = if url.starts_with("http://")
            || url.starts_with("https://")
            || url.starts_with("git@")
        {
            url.to_string()
        } else {
            format!("https://github.com/{}", url)
        };

        let mut child = tokio::process::Command::new("git")
            .args(["clone", "--progress", &full_url])
            .current_dir(&expanded)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()?;

        if let Some(stderr) = child.stderr.take() {
            let tx2 = tx.clone();
            tokio::spawn(async move {
                let mut lines = BufReader::new(stderr).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    let _ = tx2.send(line);
                }
            });
        }

        let status = child.wait().await?;
        if !status.success() {
            anyhow::bail!("git clone failed");
        }

        let name = full_url
            .split('/')
            .next_back()
            .map(|n| n.trim_end_matches(".git").to_string())
            .unwrap_or_default();
        Ok(name)
    }

    pub async fn scaffold_new(
        &self,
        name: &str,
        dir: &str,
        tx: UnboundedSender<String>,
    ) -> Result<()> {
        let expanded = expand_home(dir);
        tokio::fs::create_dir_all(&expanded).await?;

        let npx_ok = tokio::process::Command::new("which")
            .arg("npx")
            .output()
            .await
            .map(|o| o.status.success())
            .unwrap_or(false);
        if !npx_ok {
            anyhow::bail!("npx not found — install Node.js to use scaffolding");
        }

        let mut child = tokio::process::Command::new("npx")
            .args(["-y", "create-better-t-stack@latest", name, "--yes"])
            .current_dir(&expanded)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()?;

        if let Some(stdout) = child.stdout.take() {
            let tx2 = tx.clone();
            tokio::spawn(async move {
                let mut lines = BufReader::new(stdout).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    let _ = tx2.send(line);
                }
            });
        }
        if let Some(stderr) = child.stderr.take() {
            let tx2 = tx.clone();
            tokio::spawn(async move {
                let mut lines = BufReader::new(stderr).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    let _ = tx2.send(line);
                }
            });
        }

        let status = child.wait().await?;
        if !status.success() {
            anyhow::bail!("create-better-t-stack failed");
        }
        Ok(())
    }
}
