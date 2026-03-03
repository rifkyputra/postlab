use anyhow::Result;
use async_trait::async_trait;
use crate::core::models::Package;

pub mod apt;
pub mod brew;
pub mod dnf;
pub mod pacman;

pub use apt::AptManager;
pub use brew::BrewManager;
pub use dnf::DnfManager;
pub use pacman::PacmanManager;

#[async_trait]
pub trait PackageManager: Send + Sync {
    fn name(&self) -> &'static str;
    async fn list_installed(&self) -> Result<Vec<Package>>;
    async fn search(&self, query: &str) -> Result<Vec<Package>>;
    async fn install(&self, name: &str) -> Result<String>;
    async fn remove(&self, name: &str) -> Result<String>;
    async fn upgrade_all(&self) -> Result<String>;
    async fn update_cache(&self) -> Result<()>;

    /// Install with line-by-line progress forwarded to `tx`. Default falls back to `install()`.
    async fn install_streamed(
        &self,
        name: &str,
        tx: tokio::sync::mpsc::UnboundedSender<String>,
    ) -> Result<String> {
        match self.install(name).await {
            Ok(out) => {
                for line in out.lines() {
                    let _ = tx.send(line.to_string());
                }
                Ok(out)
            }
            Err(e) => {
                let msg = e.to_string();
                for line in msg.lines() {
                    let _ = tx.send(line.to_string());
                }
                Err(e)
            }
        }
    }

    /// Check installation status of specific packages. Default: full list + filter (slow).
    /// Override for targeted queries (e.g. `dpkg-query -W <names>`).
    async fn check_packages(&self, names: &[&str]) -> Result<Vec<Package>> {
        let all = self.list_installed().await?;
        let want: std::collections::HashSet<&str> = names.iter().copied().collect();
        Ok(all.into_iter().filter(|p| want.contains(p.name.as_str())).collect())
    }

    /// Remove with line-by-line progress forwarded to `tx`. Default falls back to `remove()`.
    async fn remove_streamed(
        &self,
        name: &str,
        tx: tokio::sync::mpsc::UnboundedSender<String>,
    ) -> Result<String> {
        match self.remove(name).await {
            Ok(out) => {
                for line in out.lines() {
                    let _ = tx.send(line.to_string());
                }
                Ok(out)
            }
            Err(e) => {
                let msg = e.to_string();
                for line in msg.lines() {
                    let _ = tx.send(line.to_string());
                }
                Err(e)
            }
        }
    }
}

pub struct CuratedCategory {
    pub name: &'static str,
    pub packages: &'static [&'static str],
}

pub const CURATED: &[CuratedCategory] = &[
    CuratedCategory {
        name: "Web Servers",
        packages: &["nginx", "caddy", "certbot"],
    },
    CuratedCategory {
        name: "Databases",
        packages: &["postgresql", "mariadb-server", "redis", "sqlite3"],
    },
    CuratedCategory {
        name: "System Tools",
        packages: &["htop", "tmux", "git", "curl", "wget", "vim", "rsync", "jq", "unzip"],
    },
    CuratedCategory {
        name: "Runtimes",
        packages: &["podman", "nodejs", "nvm", "pm2", "python3", "golang"],
    },
    CuratedCategory {
        name: "Security",
        packages: &["fail2ban", "ufw", "tailscale", "nmap", "unattended-upgrades"],
    },
];

pub async fn run_cmd(program: &str, args: &[&str]) -> Result<String> {
    let output = tokio::process::Command::new(program)
        .args(args)
        .output()
        .await?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        anyhow::bail!("{}", stderr)
    }
}

/// Spawn a process, stream every stdout/stderr line to `tx`, and return Ok/Err when done.
pub async fn run_cmd_streaming(
    program: &str,
    args: &[&str],
    tx: tokio::sync::mpsc::UnboundedSender<String>,
) -> Result<String> {
    use tokio::io::{AsyncBufReadExt, BufReader};

    let mut child = tokio::process::Command::new(program)
        .args(args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;

    let stdout = child.stdout.take().expect("piped stdout");
    let stderr = child.stderr.take().expect("piped stderr");

    let tx1 = tx.clone();
    let tx2 = tx.clone();

    let stdout_task = tokio::spawn(async move {
        let mut reader = BufReader::new(stdout).lines();
        while let Ok(Some(line)) = reader.next_line().await {
            let _ = tx1.send(line);
        }
    });

    let stderr_task = tokio::spawn(async move {
        let mut reader = BufReader::new(stderr).lines();
        let mut all = String::new();
        while let Ok(Some(line)) = reader.next_line().await {
            let _ = tx2.send(line.clone());
            if !all.is_empty() { all.push('\n'); }
            all.push_str(&line);
        }
        all
    });

    let status = child.wait().await?;
    let _ = stdout_task.await;
    let stderr_out = stderr_task.await.unwrap_or_default();

    if status.success() {
        Ok(String::new())
    } else {
        anyhow::bail!("{}", stderr_out)
    }
}

pub fn which(bin: &str) -> bool {
    let path_var = std::env::var_os("PATH").unwrap_or_default();
    std::env::split_paths(&path_var)
        .any(|dir| dir.join(bin).is_file())
}
