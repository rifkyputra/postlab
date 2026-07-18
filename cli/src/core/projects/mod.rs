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

#[derive(Debug, Clone, Default)]
pub struct GitStatus {
    pub installed: bool,
    pub version: String,
    pub name: String,
    pub email: String,
    pub credential_helper: String,
}

pub struct ProjectsManager;

pub fn expand_home(path: &str) -> String {
    if path.starts_with('~') {
        let home = crate::core::real_home();
        path.replacen('~', &home, 1)
    } else {
        path.to_string()
    }
}

/// Inline env for git over SSH in a non-interactive (piped) context: auto-accept a
/// first-seen host key instead of blocking on the yes/no prompt, and fail fast rather
/// than hang if a key needs a passphrase and no agent is available.
const GIT_SSH: &str =
    "GIT_SSH_COMMAND='ssh -o StrictHostKeyChecking=accept-new -o BatchMode=yes'";

/// Build a command that runs `run_cmd` as the invoking (`SUDO_USER`) user via a login
/// shell, falling back to a plain bash shell when not running under sudo. Used so git
/// operations use the user's keys, credentials and identity instead of root's.
fn user_shell_cmd(run_cmd: &str) -> tokio::process::Command {
    let user = std::env::var("SUDO_USER").unwrap_or_default();
    if user.is_empty() {
        let mut c = tokio::process::Command::new("bash");
        c.args(["-c", run_cmd]);
        c
    } else {
        let mut c = tokio::process::Command::new("su");
        c.args(["-l", &user, "-s", "/bin/bash", "-c", run_cmd]);
        c
    }
}

fn stream_to<R>(pipe: Option<R>, tx: &UnboundedSender<String>)
where
    R: tokio::io::AsyncRead + Unpin + Send + 'static,
{
    if let Some(pipe) = pipe {
        let tx = tx.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(pipe).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let _ = tx.send(line);
            }
        });
    }
}

/// Run a command as the invoking user and return trimmed stdout, or empty on failure.
async fn user_capture(run_cmd: &str) -> String {
    user_shell_cmd(run_cmd)
        .output()
        .await
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default()
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

        let full_url = if url.starts_with("http://")
            || url.starts_with("https://")
            || url.starts_with("git@")
        {
            url.to_string()
        } else {
            format!("https://github.com/{}", url)
        };

        // Run as the invoking user so the clone uses their SSH keys, credential
        // helper and git identity (root has none), and the result is user-owned.
        let mut child = user_shell_cmd(&format!(
            "mkdir -p '{expanded}' && cd '{expanded}' && {GIT_SSH} git clone --progress '{full_url}'"
        ))
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;

        stream_to(child.stdout.take(), &tx);
        stream_to(child.stderr.take(), &tx);

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

    pub async fn pull_project(&self, path: &str, tx: UnboundedSender<String>) -> Result<()> {
        // Run as the invoking user: as root, git refuses a user-owned repo with
        // "detected dubious ownership". `-C` makes su --login's CWD reset irrelevant.
        let mut child = user_shell_cmd(&format!("{GIT_SSH} git -C '{path}' pull --progress"))
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()?;

        stream_to(child.stdout.take(), &tx);
        stream_to(child.stderr.take(), &tx);

        let status = child.wait().await?;
        if !status.success() {
            anyhow::bail!("git pull failed");
        }
        Ok(())
    }

    pub async fn git_status(&self) -> GitStatus {
        let version = user_capture("git --version").await;
        GitStatus {
            installed: !version.is_empty(),
            version: version.trim_start_matches("git version ").trim().to_string(),
            name: user_capture("git config --global user.name").await,
            email: user_capture("git config --global user.email").await,
            credential_helper: user_capture("git config --global credential.helper").await,
        }
    }

    pub async fn set_git_identity(&self, name: &str, email: &str) -> Result<()> {
        // Single-quote the values; reject embedded single quotes to keep the shell safe.
        if name.contains('\'') || email.contains('\'') {
            anyhow::bail!("name/email cannot contain single quotes");
        }
        let status = user_shell_cmd(&format!(
            "git config --global user.name '{name}' && git config --global user.email '{email}'"
        ))
        .status()
        .await?;
        if !status.success() {
            anyhow::bail!("git config failed");
        }
        Ok(())
    }

    /// Configure the `store` credential helper and persist a GitHub token in
    /// `~/.git-credentials` (mode 600, user-owned) so HTTPS clones don't prompt.
    pub async fn set_github_token(&self, token: &str) -> Result<()> {
        if token.contains('\'') || token.contains('\n') {
            anyhow::bail!("invalid token");
        }
        let run = format!(
            "git config --global credential.helper store && \
             touch ~/.git-credentials && chmod 600 ~/.git-credentials && \
             grep -v 'github.com' ~/.git-credentials > ~/.git-credentials.tmp 2>/dev/null; \
             mv ~/.git-credentials.tmp ~/.git-credentials 2>/dev/null; \
             printf 'https://%s@github.com\\n' '{token}' >> ~/.git-credentials"
        );
        let status = user_shell_cmd(&run).status().await?;
        if !status.success() {
            anyhow::bail!("failed to store credentials");
        }
        Ok(())
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
        let home = crate::core::real_home();
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
        // No `--yes`: create-better-t-stack rejects it alongside explicit stack flags
        // ("use defaults" mode). Providing every flag already makes the run non-interactive.
        let run_cmd = format!("cd '{expanded}' && {shell_init}npx -y create-better-t-stack@latest '{name}' {flags}");
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
