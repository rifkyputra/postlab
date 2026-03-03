use anyhow::Result;
use async_trait::async_trait;
use crate::core::models::SshKey;
use std::path::{Path, PathBuf};
use tokio::fs;
use chrono::Local;

#[async_trait]
pub trait SshKeyManager: Send + Sync {
    async fn list_local_keys(&self) -> Result<Vec<SshKey>>;
    async fn list_authorized_keys(&self) -> Result<Vec<SshKey>>;
    async fn authorize_key(&self, key_content: &str) -> Result<()>;
    async fn deauthorize_key(&self, fingerprint: &str) -> Result<()>;
    async fn generate_key(&self, name: &str, key_type: &str) -> Result<String>;
}

pub struct DefaultSshKeyManager;

impl DefaultSshKeyManager {
    fn get_ssh_dir() -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_default();
        Path::new(&home).join(".ssh")
    }

    async fn backup_authorized_keys(&self) -> Result<()> {
        let path = Self::get_ssh_dir().join("authorized_keys");
        if path.exists() {
            let ts = Local::now().format("%Y%m%dT%H%M%S");
            let backup = path.with_extension(format!("bak.{}", ts));
            fs::copy(&path, &backup).await?;
        }
        Ok(())
    }
}

#[async_trait]
impl SshKeyManager for DefaultSshKeyManager {
    async fn list_local_keys(&self) -> Result<Vec<SshKey>> {
        let ssh_dir = Self::get_ssh_dir();
        if !ssh_dir.exists() {
            return Ok(Vec::new());
        }

        let mut keys = Vec::new();
        let mut entries = fs::read_dir(&ssh_dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("pub") {
                let content = fs::read_to_string(&path).await?;
                let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("unknown").to_string();
                
                // Try to get fingerprint using ssh-keygen -lf
                let fingerprint = match tokio::process::Command::new("ssh-keygen")
                    .args(["-lf", path.to_str().unwrap_or_default()])
                    .output()
                    .await
                {
                    Ok(out) if out.status.success() => {
                        let s = String::from_utf8_lossy(&out.stdout);
                        s.split_whitespace().nth(1).unwrap_or("unknown").to_string()
                    }
                    _ => "unknown".to_string(),
                };

                let key_type = content.split_whitespace().next().unwrap_or("unknown").to_string();

                keys.push(SshKey {
                    name,
                    fingerprint,
                    key_type,
                    content: content.trim().to_string(),
                    is_local: true,
                });
            }
        }
        Ok(keys)
    }

    async fn list_authorized_keys(&self) -> Result<Vec<SshKey>> {
        let path = Self::get_ssh_dir().join("authorized_keys");
        if !path.exists() {
            return Ok(Vec::new());
        }

        let content = fs::read_to_string(&path).await?;
        let mut keys = Vec::new();
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 2 {
                continue;
            }

            let key_type = parts[0].to_string();
            let comment = parts.get(2).map(|s| s.to_string()).unwrap_or_else(|| "no comment".to_string());

            // Get fingerprint from content string
            let mut child = tokio::process::Command::new("ssh-keygen")
                .args(["-lf", "-"])
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped())
                .spawn()?;

            if let Some(mut stdin) = child.stdin.take() {
                use tokio::io::AsyncWriteExt;
                stdin.write_all(line.as_bytes()).await?;
            }

            let out = child.wait_with_output().await?;
            let fingerprint = if out.status.success() {
                let s = String::from_utf8_lossy(&out.stdout);
                s.split_whitespace().nth(1).unwrap_or("unknown").to_string()
            } else {
                "unknown".to_string()
            };

            keys.push(SshKey {
                name: comment,
                fingerprint,
                key_type,
                content: line.to_string(),
                is_local: false,
            });
        }
        Ok(keys)
    }

    async fn authorize_key(&self, key_content: &str) -> Result<()> {
        self.backup_authorized_keys().await?;
        let ssh_dir = Self::get_ssh_dir();
        fs::create_dir_all(&ssh_dir).await?;
        let path = ssh_dir.join("authorized_keys");
        
        // Check if already exists
        if path.exists() {
            let existing = fs::read_to_string(&path).await?;
            if existing.contains(key_content.trim()) {
                return Ok(());
            }
        }

        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .await?;
        
        use tokio::io::AsyncWriteExt;
        if path.metadata()?.len() > 0 {
            file.write_all(b"\n").await?;
        }
        file.write_all(key_content.trim().as_bytes()).await?;
        file.write_all(b"\n").await?;
        
        // Ensure 600 permissions
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).await?;
        }

        Ok(())
    }

    async fn deauthorize_key(&self, fingerprint: &str) -> Result<()> {
        self.backup_authorized_keys().await?;
        let path = Self::get_ssh_dir().join("authorized_keys");
        if !path.exists() {
            return Ok(());
        }

        let content = fs::read_to_string(&path).await?;
        let mut lines = Vec::new();
        for line in content.lines() {
            let line_trimmed = line.trim();
            if line_trimmed.is_empty() || line_trimmed.starts_with('#') {
                lines.push(line.to_string());
                continue;
            }

            // Check fingerprint
            let mut child = tokio::process::Command::new("ssh-keygen")
                .args(["-lf", "-"])
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped())
                .spawn()?;

            if let Some(mut stdin) = child.stdin.take() {
                use tokio::io::AsyncWriteExt;
                stdin.write_all(line_trimmed.as_bytes()).await?;
            }

            let out = child.wait_with_output().await?;
            let f = if out.status.success() {
                let s = String::from_utf8_lossy(&out.stdout);
                s.split_whitespace().nth(1).unwrap_or("unknown").to_string()
            } else {
                "unknown".to_string()
            };

            if f != fingerprint {
                lines.push(line.to_string());
            }
        }

        fs::write(&path, lines.join("\n")).await?;
        Ok(())
    }

    async fn generate_key(&self, name: &str, key_type: &str) -> Result<String> {
        let ssh_dir = Self::get_ssh_dir();
        fs::create_dir_all(&ssh_dir).await?;
        let path = ssh_dir.join(name);
        
        if path.exists() {
            anyhow::bail!("Key file already exists: {}", path.display());
        }

        let out = tokio::process::Command::new("ssh-keygen")
            .args(["-t", key_type, "-f", path.to_str().unwrap_or_default(), "-N", ""])
            .output()
            .await?;

        if out.status.success() {
            Ok(String::from_utf8_lossy(&out.stdout).to_string())
        } else {
            anyhow::bail!("ssh-keygen failed: {}", String::from_utf8_lossy(&out.stderr))
        }
    }
}
