#![allow(dead_code)]

use std::path::PathBuf;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::mpsc::UnboundedSender;

use super::creds::{GitCreds, RunAs};
use super::error::GitError;

pub struct GitInstall {
    pub installed: bool,
    pub version: String,
}

#[derive(Debug)]
pub enum PullResult {
    FastForwarded { from: String, to: String },
    AlreadyUpToDate,
}

pub struct GitRepo {
    pub path: PathBuf,
    pub remote: String,
    pub creds: GitCreds,
    pub run_as: RunAs,
}

impl GitRepo {
    pub fn new(path: PathBuf, remote: String, creds: GitCreds, run_as: RunAs) -> Self {
        Self { path, remote, creds, run_as }
    }

    pub async fn install_status() -> GitInstall {
        match tokio::process::Command::new("git")
            .arg("--version")
            .env("GIT_TERMINAL_PROMPT", "0")
            .output()
            .await
        {
            Ok(o) if o.status.success() => GitInstall {
                installed: true,
                version: String::from_utf8_lossy(&o.stdout)
                    .trim_start_matches("git version ")
                    .trim()
                    .to_string(),
            },
            _ => GitInstall { installed: false, version: String::new() },
        }
    }

    /// Clone `remote` into `path`. Creates `path` if absent.
    pub async fn clone(
        &self,
        branch: Option<&str>,
        depth: Option<u32>,
        tx: &UnboundedSender<String>,
    ) -> Result<(), GitError> {
        self.check_installed().await?;
        tokio::fs::create_dir_all(&self.path).await?;

        let mut args: Vec<&str> = vec!["clone", "--progress"];
        let branch_owned;
        if let Some(b) = branch {
            branch_owned = b.to_string();
            args.extend(["--branch", &branch_owned]);
        }
        let depth_owned;
        if let Some(d) = depth {
            depth_owned = d.to_string();
            args.extend(["--depth", &depth_owned]);
        }
        // Clone remote into the current dir (self.path, which we just created).
        let remote = self.remote.clone();
        args.push(&remote);
        args.push(".");

        let (ok, stderr) = self.run_streaming(&args, tx).await?;
        if !ok {
            return Err(classify_error(&stderr));
        }
        Ok(())
    }

    /// Fetch then `pull --ff-only`. Returns `DirtyTree` if the working tree has
    /// tracked modifications and `force` is false; with `force`, resets first.
    pub async fn pull_ff_only(
        &self,
        force: bool,
        tx: &UnboundedSender<String>,
    ) -> Result<PullResult, GitError> {
        self.check_installed().await?;
        if force {
            self.run_silent(&["reset", "--hard", "HEAD"]).await?;
        } else if self.is_dirty().await? {
            return Err(GitError::DirtyTree);
        }
        let before = self.current_sha().await?;
        let (ok, stderr) = self.run_streaming(&["pull", "--ff-only", "--progress"], tx).await?;
        if !ok {
            return Err(classify_error(&stderr));
        }
        let after = self.current_sha().await?;
        if before == after {
            Ok(PullResult::AlreadyUpToDate)
        } else {
            Ok(PullResult::FastForwarded { from: before, to: after })
        }
    }

    /// Fetch then checkout `rev` (branch, tag, or SHA) in detached HEAD.
    /// With `force`, resets and cleans the working tree first — safe because
    /// generated artifacts live in `generated/`, outside this tree.
    pub async fn checkout(
        &self,
        rev: &str,
        force: bool,
        tx: &UnboundedSender<String>,
    ) -> Result<(), GitError> {
        self.check_installed().await?;
        if force {
            self.run_silent(&["reset", "--hard", "HEAD"]).await?;
            self.run_silent(&["clean", "-fd"]).await?;
        }
        let (ok, stderr) = self.run_streaming(&["checkout", rev], tx).await?;
        if !ok {
            return Err(classify_error(&stderr));
        }
        Ok(())
    }

    pub async fn current_sha(&self) -> Result<String, GitError> {
        let out = self.base_cmd()
            .current_dir(&self.path)
            .args(["rev-parse", "HEAD"])
            .output()
            .await?;
        if !out.status.success() {
            return Err(GitError::Io(std::io::Error::other("git rev-parse HEAD failed")));
        }
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    }

    /// Returns true when tracked files have modifications (excludes untracked).
    pub async fn is_dirty(&self) -> Result<bool, GitError> {
        let out = self.base_cmd()
            .current_dir(&self.path)
            .args(["status", "--porcelain"])
            .output()
            .await?;
        if !out.status.success() {
            return Err(GitError::Io(std::io::Error::other("git status failed")));
        }
        Ok(!out.stdout.trim_ascii().is_empty())
    }

    /// SHA of `origin/<branch>` as seen after the last fetch.
    pub async fn remote_sha(&self, branch: &str) -> Result<String, GitError> {
        let refspec = format!("refs/remotes/origin/{branch}");
        let out = self.base_cmd()
            .current_dir(&self.path)
            .args(["rev-parse", &refspec])
            .output()
            .await?;
        if !out.status.success() {
            return Err(GitError::RemoteNotFound);
        }
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    }

    // ── private ──────────────────────────────────────────────────────

    fn base_cmd(&self) -> tokio::process::Command {
        let mut cmd = tokio::process::Command::new("git");
        cmd.env("GIT_TERMINAL_PROMPT", "0");

        let ssh_cmd = match &self.creds {
            GitCreds::SshKey { private_key_path } => format!(
                "ssh -i {} -o StrictHostKeyChecking=accept-new -o BatchMode=yes",
                private_key_path.display()
            ),
            _ => "ssh -o StrictHostKeyChecking=accept-new -o BatchMode=yes".to_string(),
        };
        cmd.env("GIT_SSH_COMMAND", &ssh_cmd);

        match &self.run_as {
            RunAs::Root => {
                cmd.env("GIT_CONFIG_GLOBAL", "/var/lib/postlab/.gitconfig");
            }
            RunAs::User(uid) => {
                let uid = *uid;
                // Safety: pre_exec runs in the forked child before exec.
                // Calling setuid here drops root before git starts.
                unsafe {
                    cmd.pre_exec(move || {
                        nix::unistd::setuid(nix::unistd::Uid::from_raw(uid))
                            .map_err(std::io::Error::from)?;
                        Ok(())
                    });
                }
            }
        }

        cmd
    }

    async fn check_installed(&self) -> Result<(), GitError> {
        match tokio::process::Command::new("git")
            .arg("--version")
            .env("GIT_TERMINAL_PROMPT", "0")
            .output()
            .await
        {
            Ok(o) if o.status.success() => Ok(()),
            Ok(_) => Err(GitError::NotInstalled),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(GitError::NotInstalled),
            Err(e) => Err(GitError::Io(e)),
        }
    }

    /// Run git with no output capture; used for reset/clean where we don't
    /// stream progress.
    async fn run_silent(&self, args: &[&str]) -> Result<(), GitError> {
        let status = self.base_cmd()
            .current_dir(&self.path)
            .args(args)
            .status()
            .await?;
        if !status.success() {
            return Err(GitError::Io(std::io::Error::other(format!(
                "git {} exited {}",
                args.first().copied().unwrap_or(""),
                status.code().unwrap_or(-1)
            ))));
        }
        Ok(())
    }

    /// Run git in `self.path`, stream both stdout and stderr to `tx`, collect
    /// stderr for error classification, and return `(success, stderr)`.
    async fn run_streaming(
        &self,
        args: &[&str],
        tx: &UnboundedSender<String>,
    ) -> Result<(bool, String), GitError> {
        let mut child = self.base_cmd()
            .current_dir(&self.path)
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        let stdout = child.stdout.take();
        let stderr_pipe = child.stderr.take().expect("stderr piped");

        // Stream stdout in background (fire and forget).
        if let Some(r) = stdout {
            let tx2 = tx.clone();
            tokio::spawn(async move {
                let mut lines = BufReader::new(r).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    let _ = tx2.send(line);
                }
            });
        }

        // Collect stderr while also streaming it to tx.
        let tx2 = tx.clone();
        let stderr_task = tokio::spawn(async move {
            let mut lines = BufReader::new(stderr_pipe).lines();
            let mut buf = String::new();
            while let Ok(Some(line)) = lines.next_line().await {
                let _ = tx2.send(line.clone());
                buf.push_str(&line);
                buf.push('\n');
            }
            buf
        });

        let status = child.wait().await?;
        let stderr = stderr_task.await.unwrap_or_default();
        Ok((status.success(), stderr))
    }
}

fn classify_error(stderr: &str) -> GitError {
    let s = stderr.to_lowercase();
    if s.contains("not possible to fast-forward") || s.contains("cannot fast-forward") {
        GitError::FastForwardRejected
    } else if s.contains("authentication failed")
        || s.contains("could not read username")
        || s.contains("invalid credentials")
    {
        GitError::AuthFailed
    } else if s.contains("host key verification failed") || s.contains("known_hosts") {
        GitError::HostKeyRejected
    } else if s.contains("repository not found")
        || (s.contains("fatal") && s.contains("not found"))
    {
        GitError::RemoteNotFound
    } else {
        GitError::Io(std::io::Error::other(stderr.trim().to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use tokio::sync::mpsc;

    async fn init_remote() -> (TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().to_path_buf();
        git(&p, &["init"]).await;
        git(&p, &["config", "user.email", "ci@postlab.test"]).await;
        git(&p, &["config", "user.name", "CI"]).await;
        tokio::fs::write(p.join("README.md"), "init").await.unwrap();
        git(&p, &["add", "."]).await;
        git(&p, &["commit", "-m", "init"]).await;
        (dir, p)
    }

    async fn git(dir: &std::path::Path, args: &[&str]) {
        tokio::process::Command::new("git")
            .current_dir(dir)
            .args(args)
            .env("GIT_TERMINAL_PROMPT", "0")
            .output()
            .await
            .unwrap();
    }

    fn repo(remote: &std::path::Path, target: &std::path::Path) -> GitRepo {
        GitRepo::new(
            target.to_path_buf(),
            format!("file://{}", remote.display()),
            GitCreds::None,
            RunAs::Root,
        )
    }

    fn tx() -> UnboundedSender<String> {
        mpsc::unbounded_channel::<String>().0
    }

    #[tokio::test]
    async fn install_status_reports_installed() {
        let s = GitRepo::install_status().await;
        assert!(s.installed);
        assert!(!s.version.is_empty());
    }

    #[tokio::test]
    async fn clone_basic() {
        let (_remote, rp) = init_remote().await;
        let target = tempfile::tempdir().unwrap();
        repo(&rp, target.path()).clone(None, None, &tx()).await.unwrap();
        assert!(target.path().join(".git").exists());
    }

    #[tokio::test]
    async fn clone_with_depth() {
        let (_remote, rp) = init_remote().await;
        let target = tempfile::tempdir().unwrap();
        repo(&rp, target.path()).clone(None, Some(1), &tx()).await.unwrap();
        assert!(target.path().join(".git").exists());
    }

    #[tokio::test]
    async fn pull_ff_already_up_to_date() {
        let (_remote, rp) = init_remote().await;
        let target = tempfile::tempdir().unwrap();
        let r = repo(&rp, target.path());
        r.clone(None, None, &tx()).await.unwrap();
        let result = r.pull_ff_only(false, &tx()).await.unwrap();
        assert!(matches!(result, PullResult::AlreadyUpToDate));
    }

    #[tokio::test]
    async fn pull_ff_fast_forwards() {
        let (_remote, rp) = init_remote().await;
        let target = tempfile::tempdir().unwrap();
        let r = repo(&rp, target.path());
        r.clone(None, None, &tx()).await.unwrap();

        tokio::fs::write(rp.join("v2.txt"), "v2").await.unwrap();
        git(&rp, &["add", "."]).await;
        git(&rp, &["commit", "-m", "v2"]).await;

        let before = r.current_sha().await.unwrap();
        let result = r.pull_ff_only(false, &tx()).await.unwrap();
        let after = r.current_sha().await.unwrap();

        assert!(matches!(result, PullResult::FastForwarded { .. }));
        assert_ne!(before, after);
    }

    #[tokio::test]
    async fn pull_ff_dirty_tree_rejected() {
        let (_remote, rp) = init_remote().await;
        let target = tempfile::tempdir().unwrap();
        let r = repo(&rp, target.path());
        r.clone(None, None, &tx()).await.unwrap();

        tokio::fs::write(target.path().join("README.md"), "dirty").await.unwrap();

        let err = r.pull_ff_only(false, &tx()).await.unwrap_err();
        assert!(matches!(err, GitError::DirtyTree));
    }

    #[tokio::test]
    async fn pull_ff_diverged_rejected() {
        let (_remote, rp) = init_remote().await;
        let target = tempfile::tempdir().unwrap();
        let r = repo(&rp, target.path());
        r.clone(None, None, &tx()).await.unwrap();

        // Create a local commit in the clone so clone/main diverges from origin/main.
        let tp = target.path();
        git(tp, &["config", "user.email", "ci@postlab.test"]).await;
        git(tp, &["config", "user.name", "CI"]).await;
        tokio::fs::write(tp.join("local.txt"), "local").await.unwrap();
        git(tp, &["add", "."]).await;
        git(tp, &["commit", "-m", "local"]).await;

        // Add a different commit to remote so the histories diverge.
        tokio::fs::write(rp.join("remote.txt"), "remote").await.unwrap();
        git(&rp, &["add", "."]).await;
        git(&rp, &["commit", "-m", "remote"]).await;

        let err = r.pull_ff_only(false, &tx()).await.unwrap_err();
        assert!(matches!(err, GitError::FastForwardRejected));
    }

    #[tokio::test]
    async fn checkout_to_prior_sha_and_back() {
        let (_remote, rp) = init_remote().await;
        let target = tempfile::tempdir().unwrap();
        let r = repo(&rp, target.path());
        r.clone(None, None, &tx()).await.unwrap();
        let sha1 = r.current_sha().await.unwrap();

        tokio::fs::write(rp.join("v2.txt"), "v2").await.unwrap();
        git(&rp, &["add", "."]).await;
        git(&rp, &["commit", "-m", "v2"]).await;
        r.pull_ff_only(false, &tx()).await.unwrap();
        let sha2 = r.current_sha().await.unwrap();
        assert_ne!(sha1, sha2);

        r.checkout(&sha1, false, &tx()).await.unwrap();
        assert_eq!(r.current_sha().await.unwrap(), sha1);
    }

    #[tokio::test]
    async fn is_dirty_detects_modification() {
        let (_remote, rp) = init_remote().await;
        let target = tempfile::tempdir().unwrap();
        let r = repo(&rp, target.path());
        r.clone(None, None, &tx()).await.unwrap();

        assert!(!r.is_dirty().await.unwrap());
        tokio::fs::write(target.path().join("README.md"), "changed").await.unwrap();
        assert!(r.is_dirty().await.unwrap());
    }
}
