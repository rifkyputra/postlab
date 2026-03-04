use anyhow::Result;
use async_trait::async_trait;
use tokio::fs;
use tokio::process::Command;
use crate::core::models::UserInfo;

#[async_trait]
pub trait UserManager: Send + Sync {
    async fn list_users(&self) -> Result<Vec<UserInfo>>;
    async fn create_user(&self, username: &str, shell: Option<&str>) -> Result<()>;
    async fn delete_user(&self, username: &str) -> Result<()>;
}

pub struct UnixUserManager;

#[async_trait]
impl UserManager for UnixUserManager {
    async fn list_users(&self) -> Result<Vec<UserInfo>> {
        let content = fs::read_to_string("/etc/passwd").await?;
        let mut users = Vec::new();

        for line in content.lines() {
            if line.starts_with('#') || line.is_empty() {
                continue;
            }

            let parts: Vec<&str> = line.split(':').collect();
            if parts.len() >= 7 {
                let username = parts[0].to_string();
                let uid = parts[2].parse::<u32>().unwrap_or(0);
                let gid = parts[3].parse::<u32>().unwrap_or(0);
                let home = parts[5].to_string();
                let shell = parts[6].to_string();

                // Skip system users (usually UID < 1000, except on macOS where they are low too)
                // On macOS, human users usually start at 501.
                // On Linux, human users usually start at 1000.
                if uid < 500 && username != "root" {
                    continue;
                }

                users.push(UserInfo {
                    username,
                    uid,
                    gid,
                    home,
                    shell,
                    groups: Vec::new(), // We'll fetch groups if needed or in a separate pass
                });
            }
        }

        // Fetch groups for each user
        for user in &mut users {
            let output = Command::new("id")
                .args(["-Gn", &user.username])
                .output()
                .await;

            if let Ok(out) = output {
                let groups_str = String::from_utf8_lossy(&out.stdout);
                user.groups = groups_str
                    .split_whitespace()
                    .map(|s| s.to_string())
                    .collect();
            }
        }

        // Sort by username
        users.sort_by(|a, b| a.username.cmp(&b.username));

        Ok(users)
    }

    async fn create_user(&self, username: &str, shell: Option<&str>) -> Result<()> {
        let mut cmd = Command::new("useradd");
        cmd.arg("-m"); // Create home directory
        if let Some(s) = shell {
            cmd.arg("-s").arg(s);
        }
        cmd.arg(username);

        let status = cmd.status().await?;
        if !status.success() {
            anyhow::bail!("Failed to create user: {}", username);
        }
        Ok(())
    }

    async fn delete_user(&self, username: &str) -> Result<()> {
        let status = Command::new("userdel")
            .arg("-r") // Remove home directory
            .arg(username)
            .status()
            .await?;

        if !status.success() {
            anyhow::bail!("Failed to delete user: {}", username);
        }
        Ok(())
    }
}
