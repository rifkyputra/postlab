use anyhow::Result;
use std::cmp::Reverse;
use std::path::PathBuf;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::mpsc::UnboundedSender;

#[derive(Debug, Clone)]
pub struct ProjectEntry {
    pub name: String,
    pub path: String,
    pub stack: String,
    pub modified: Option<u64>,
}

fn detect_stack(path: &std::path::Path) -> String {
    let checks: &[(&[&str], &str)] = &[
        (&["wadm.yaml", "wasmcloud.toml"], "WasmCloud"),
        (&["Cargo.toml"], "Rust"),
        (&["go.mod"], "Go"),
        (&["package.json", "package-lock.json", "yarn.lock", "pnpm-lock.yaml"], "Node"),
        (&["pyproject.toml", "requirements.txt", "setup.py"], "Python"),
        (&["docker-compose.yml", "docker-compose.yaml", "compose.yaml", "compose.yml", "Dockerfile"], "Docker"),
    ];
    for (files, label) in checks {
        if files.iter().any(|f| path.join(f).exists()) {
            return label.to_string();
        }
    }
    "—".to_string()
}

pub struct ProjectsManager;

pub fn expand_home(path: &str) -> String {
    if path.starts_with('~') {
        let home = std::env::var("SUDO_USER")
            .ok()
            .map(|u| format!("/home/{}", u))
            .unwrap_or_else(|| std::env::var("HOME").unwrap_or_else(|_| "/root".to_string()));
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
            let entry_path = entry.path();
            let stack = detect_stack(&entry_path);
            entries.push(ProjectEntry {
                name: entry.file_name().to_string_lossy().to_string(),
                path: entry_path.to_string_lossy().to_string(),
                stack,
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

    /// `flags` is appended verbatim after the project name, e.g.
    /// `"--frontend next --database sqlite --orm drizzle --auth better-auth --backend hono --api trpc"`.
    pub async fn scaffold_new(
        &self,
        name: &str,
        dir: &str,
        flags: &str,
        tx: UnboundedSender<String>,
    ) -> Result<()> {
        let expanded = expand_home(dir);

        let user = std::env::var("SUDO_USER").unwrap_or_default();
        let home = if user.is_empty() {
            std::env::var("HOME").unwrap_or_else(|_| "/root".to_string())
        } else {
            format!("/home/{user}")
        };
        // .bashrc has an interactivity guard and can't be sourced from a non-interactive
        // process. Instead: init nvm directly (its script has no guard) and prepend the
        // well-known bin dirs used by nvm, pi-node, volta, and npm-global.
        let shell_init = format!(
            "export NVM_DIR=\"${{NVM_DIR:-{home}/.nvm}}\"; \
             [ -s \"$NVM_DIR/nvm.sh\" ] && . \"$NVM_DIR/nvm.sh\"; \
             export PATH=\"{home}/.local/share/pi-node/current/bin\
             :{home}/.volta/bin\
             :{home}/.npm-global/bin\
             :{home}/.local/bin\
             :/usr/local/bin\
             :$PATH\"; "
        );
        let npx_ok = if user.is_empty() {
            tokio::process::Command::new("bash")
                .args(["-c", &format!("{shell_init}command -v npx")])
                .output()
                .await
                .map(|o| o.status.success())
                .unwrap_or(false)
        } else {
            tokio::process::Command::new("su")
                .args(["-l", &user, "-s", "/bin/bash", "-c", &format!("{shell_init}command -v npx")])
                .output()
                .await
                .map(|o| o.status.success())
                .unwrap_or(false)
        };
        if !npx_ok {
            anyhow::bail!("npx not found — install Node.js to use scaffolding");
        }

        // Ensure the projects dir exists and is owned by the invoking user *before*
        // dropping privileges. Older builds created it via create_dir_all as root, which
        // left it root-owned — npx then fails with EACCES when it tries to mkdir the
        // project subdirectory as the unprivileged user.
        if user.is_empty() {
            tokio::fs::create_dir_all(&expanded).await?;
        } else {
            tokio::process::Command::new("mkdir")
                .args(["-p", &expanded])
                .status()
                .await?;
            tokio::process::Command::new("chown")
                .args([&user, &expanded])
                .status()
                .await?;
        }

        // su --login resets CWD to the user's home; cd into the configured dir explicitly.
        let run_cmd = format!("cd '{expanded}' && {shell_init}npx -y create-better-t-stack@latest '{name}' {flags} --yes");
        let mut cmd = if user.is_empty() {
            let mut c = tokio::process::Command::new("bash");
            c.args(["-c", &run_cmd]);
            c
        } else {
            let mut c = tokio::process::Command::new("su");
            c.args(["-l", &user, "-s", "/bin/bash", "-c", &run_cmd]);
            c
        };
        let mut child = cmd
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

#[cfg(test)]
mod tests {
    use super::{expand_home, ProjectsManager};
    use std::fs;

    #[test]
    fn expand_home_leaves_absolute_paths_unchanged() {
        let path = "/tmp/foo/bar";
        assert_eq!(expand_home(path), path);
    }

    #[test]
    fn expand_home_replaces_tilde_prefix() {
        let expanded = expand_home("~/projects");
        assert!(!expanded.starts_with('~'), "tilde should be replaced");
        assert!(expanded.ends_with("/projects"));
    }

    #[test]
    fn expand_home_only_replaces_leading_tilde() {
        let path = "/opt/~weird";
        assert_eq!(expand_home(path), path);
    }

    #[tokio::test]
    async fn list_returns_empty_for_missing_dir() {
        let result = ProjectsManager.list("/nonexistent/path/xyz123").await.unwrap();
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn list_returns_only_directories() {
        let root = std::env::temp_dir().join(format!(
            "postlab-projects-list-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(root.join("project-a")).unwrap();
        fs::create_dir_all(root.join("project-b")).unwrap();
        fs::write(root.join("README.md"), "").unwrap();

        let entries = ProjectsManager.list(root.to_str().unwrap()).await.unwrap();
        assert_eq!(entries.len(), 2);
        let names: Vec<_> = entries.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"project-a"));
        assert!(names.contains(&"project-b"));
    }
}
