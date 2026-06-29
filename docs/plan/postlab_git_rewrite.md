# postlab git — Git wrapper rewrite

## Goal

Replace the half-baked `core/deploy/git.rs` stubs and the duplicated shell logic in `core/projects/mod.rs` with a single, testable, root-friendly Git wrapper that `postlab go` can rely on for clone, fast-forward pull, checkout-by-SHA, dirty-tree detection, and authenticated remotes.

## Why now

- `core/deploy/git.rs` is dead code with two functions: `clone_repo` and `pull_repo`.
- `pull_repo` does a plain `git pull`, not `git pull --ff-only` as `postlab go` requires.
- There is no dirty-tree detection, no rollback checkout, no branch/SHA support, no auth handling, and no progress streaming.
- `core/projects/mod.rs` reimplements clone/pull/status/identity/token in shell strings, with manual quoting and `su` gymnastics. That logic should live in one place and be reused by both the project browser and the deploy pipeline.
- On a fresh Linux install `git` itself may be missing; the wrapper must integrate with the prerequisite harness being planned for `postlab go`.

## Non-goals

- Do not switch to `git2-rs` or `gix` (gitoxide). Both add native dependencies or auth complexity that conflict with the static-binary/cross-compile target.
- Do not implement full `git` porcelain (no commit/push/merge/rebase). Postlab only needs read-only remote sync plus force-checkout for rollback.
- Do not change the repo-wide schema mechanism (inline `CREATE TABLE IF NOT EXISTS` in `db/mod.rs`).

## Decision: keep the `git` CLI, wrap it properly

Shelling out to the `git` CLI is the right trade-off for postlab:

- It handles SSH, HTTPS tokens, credential helpers, LFS, and submodules for free.
- It is easy to install on every supported platform via existing package adapters.
- Static builds stay simple because we do not link `libgit2`.

The wrapper's job is to remove shell fragility and give typed semantics.

## Proposed API

New module: `cli/src/core/git/`.

```
cli/src/core/git/
├── mod.rs       # public exports
├── repo.rs      # GitRepo struct and operations
├── creds.rs     # GitCreds, key files, credential helper
└── error.rs     # GitError enum
```

### Core types

```rust
pub struct GitRepo {
    pub path: PathBuf,
    pub remote: String,
    pub creds: GitCreds,
}

pub enum GitCreds {
    None,
    HttpsToken { host: String, token: String },
    SshKey { private_key_path: PathBuf },
}

pub enum PullResult {
    FastForwarded { from: String, to: String },
    AlreadyUpToDate,
}

pub struct GitStatus {
    pub installed: bool,
    pub version: String,
}
```

### Operations

```rust
impl GitRepo {
    pub fn new(path: PathBuf, remote: String, creds: GitCreds) -> Self;

    /// Clone remote into `path`. Streams progress lines to `tx`.
    pub async fn clone(&self, tx: &mpsc::UnboundedSender<String>) -> Result<()>;

    /// Fetch then pull --ff-only. Fails if working tree is dirty unless `force` is true.
    pub async fn pull_ff_only(&self, force: bool, tx: &mpsc::UnboundedSender<String>) -> Result<PullResult>;

    /// Fetch then checkout `rev` (branch, tag, or SHA) in detached HEAD. Cleans untracked files.
    pub async fn checkout(&self, rev: &str, force: bool, tx: &mpsc::UnboundedSender<String>) -> Result<()>;

    pub async fn current_sha(&self) -> Result<String>;
    pub async fn is_dirty(&self) -> Result<bool>;
    pub async fn remote_sha(&self, branch: &str) -> Result<String>;
    pub async fn has_untracked(&self) -> Result<bool>;
}
```

## Authentication & key management

### Credentials strategy

| Remote type | Storage | How it is used |
|---|---|---|
| Public HTTPS | none | Plain clone |
| Private HTTPS (token) | SQLite app row or per-app `.git-credentials` | `https://<token>@host/...` via credential helper |
| Private SSH | per-app ed25519 key file | `GIT_SSH_COMMAND='ssh -i <key> -o StrictHostKeyChecking=accept-new -o BatchMode=yes'` |

### Key files

- Private keys live in `/var/lib/postlab/apps/<id>/.git/deploy_key` with mode `0600`, owned by root.
- Public keys can be displayed with a CLI command so the user can add them to GitHub/GitLab.
- Known hosts are managed in `/var/lib/postlab/.ssh/known_hosts`, not the invoking user's home, because the deploy runs as root.

### CLI commands

```bash
postlab git deploy-key --app <id>          # generate or show ed25519 deploy key
postlab git allow-host <host>              # ssh-keyscan host into managed known_hosts
postlab git set-token --app <id> <token>   # store HTTPS token for the app
```

These mirror Dokku's `git:generate-deploy-key`, `git:allow-host`, and `git:auth` commands.

## Environment hardening

Every `git` invocation gets this base environment so it cannot hang or prompt interactively:

```rust
Command::new("git")
    .env("GIT_TERMINAL_PROMPT", "0")
    .env("GIT_SSH_COMMAND", ssh_cmd)
    .env("GIT_CONFIG_GLOBAL", global_config_path)
    .current_dir(&self.path)
```

- `GIT_TERMINAL_PROMPT=0` fails fast when credentials are missing.
- `GIT_SSH_COMMAND` points to the app key and accepts new host keys automatically.
- `GIT_CONFIG_GLOBAL` points to `/var/lib/postlab/.gitconfig` so git identity and credential helper are controlled by postlab, not the root user's personal config.

## Integration with `postlab go`

`AppManager::deploy()` will use the wrapper like this:

```rust
let repo = GitRepo::new(config_dir.into(), app.repo_url.clone(), creds);

if !repo.path.join(".git").exists() {
    repo.clone(&tx).await?;
} else {
    match repo.pull_ff_only(force, &tx).await? {
        PullResult::AlreadyUpToDate => {}
        PullResult::FastForwarded { from, to } => {
            info!("pulled {}..{}", from, to);
        }
    }
}

let sha = repo.current_sha().await?;
```

Rollback will call `repo.checkout(previous_sha, true, &tx)`.

The `core/projects/mod.rs` clone/pull/status logic will be refactored to call `GitRepo` instead of building shell strings.

## Fresh-Linux integration

The wrapper depends on the prerequisite harness planned in `docs/research/postlab_go_tooling_problems.md`:

- `git` itself is checked by `postlab go --doctor`.
- If `git` is missing, the package adapter for the detected platform installs it (`git` on apt/dnf/pacman).
- `ssh-keygen` is pulled in transitively by the `openssh-client` / `openssh` package on every supported distro.
- Because we shell out to `git`, no extra Rust build dependencies are introduced.

## Error handling

Convert raw git exit statuses into actionable errors:

```rust
pub enum GitError {
    NotInstalled,
    DirtyTree,
    FastForwardRejected,
    AuthFailed,
    HostKeyRejected,
    RemoteNotFound,
    Io(#[from] std::io::Error),
}
```

Error messages propagate through the deploy WebSocket so the UI can show "Working tree has local changes" rather than "git pull failed with status 1".

## Phases

### Phase 1 — Core wrapper (foundational)

- Create `cli/src/core/git/` with `repo.rs`, `creds.rs`, `error.rs`.
- Implement `GitRepo::new`, `clone`, `pull_ff_only`, `checkout`, `current_sha`, `is_dirty`.
- Wire progress streaming through `mpsc::UnboundedSender<String>`.
- Add `GitError` with distinct variants.
- Replace `core/deploy/git.rs` with re-exports or delete it after Phase 2.

### Phase 2 — Auth & keys

- Add `GitCreds::HttpsToken` and `GitCreds::SshKey`.
- Implement per-app key generation and storage.
- Implement `postlab git deploy-key` and `postlab git allow-host`.
- Implement `postlab git set-token`.

### Phase 3 — Refactor existing callers

- Rewrite `core/projects/mod.rs::clone_repo` to use `GitRepo::clone`.
- Rewrite `core/projects/mod.rs::pull_project` to use `GitRepo::pull_ff_only`.
- Keep `git_status` / `set_git_identity` / `set_github_token` but delegate file/SSH setup to `core/git/creds.rs`.
- Remove duplicated shell-quoting logic.

### Phase 4 — `postlab go` integration

- `AppManager::deploy()` uses `GitRepo`.
- Rollback uses `GitRepo::checkout`.
- Deploy log records the resolved SHA.
- Webhook-triggered deploys pass the pushed SHA to `checkout` when it differs from `HEAD`.

### Phase 5 — Tooling & doctor

- Add `git` to the prerequisite harness.
- `postlab go --doctor` reports git status.
- Package adapters can install `git` if missing.

## Testing

- Unit tests with a local bare repo created in `tempfile::tempdir()`:
  - clone into empty dir
  - `pull_ff_only` succeeds on fast-forward
  - `pull_ff_only` fails on dirty tree
  - `checkout` to previous commit and back
- Integration test on a fresh Ubuntu VM:
  - install postlab binary
  - run `postlab git deploy-key --app test`
  - clone a private GitHub repo using the deploy key
  - pull a new commit and verify `current_sha`

## Risks & open questions

1. **Root-owned repos vs. user-owned repos.** The plan opts for root-owned `/var/lib/postlab/apps/*` because `postlab go` runs as root. If users later want user-level project browsing to share keys, `core/projects/mod.rs` may need a separate user-mode `GitRepo` with `SUDO_USER` fallback.
2. **SSH host key policy.** `StrictHostKeyChecking=accept-new` is convenient but allows first-connect MITM. A stricter mode could be offered later.
3. **LFS and submodules.** Out of scope for v1. If needed, add `fetch --all` and `submodule update --init --recursive` behind explicit flags.
4. **Credential storage in SQLite.** HTTPS tokens will be stored encrypted or in per-app files, not plain DB rows. The exact mechanism should match the repo's existing secret-handling pattern.

## Success criteria

- `make check && make test` pass with zero warnings.
- `core/deploy/git.rs` no longer contains dead stubs.
- `core/projects/mod.rs` no longer shells out to `git` directly.
- `postlab go` deploys from a private repo using either HTTPS token or SSH deploy key on a fresh Linux VM.
