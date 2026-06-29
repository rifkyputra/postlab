# postlab git — Git wrapper rewrite

> **Sequencing:** this is **Phase 1 of [`postlab_go_roadmap.md`](./postlab_go_roadmap.md)** —
> the first feature built, before the web server, schema, or any backend, because every deploy,
> rollback, and webhook starts with a git operation. The roadmap's Phase 1a–1c map onto Phases
> 1–3 below; the AppManager/doctor integration (Phases 4–5 below) lands inside the roadmap
> phases that own those components (P5 walking skeleton, P4 prereq harness).

## Goal

Replace the half-baked `core/deploy/git.rs` stubs and the duplicated shell logic in `core/projects/mod.rs` with a single, testable Git wrapper that `postlab go` can rely on for clone, fast-forward pull, checkout-by-SHA, dirty-tree detection, and authenticated remotes — supporting **both** root-owned deploy repos and user-owned project-browser repos.

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
    pub path: PathBuf,        // the working tree (clone target), e.g. /var/lib/postlab/apps/<id>/repo
    pub remote: String,
    pub creds: GitCreds,
    pub run_as: RunAs,        // identity + ownership — see below
}

/// The single decision that cannot be retrofitted later: who owns the checkout.
pub enum RunAs {
    Root,        // deploy pipeline: root-owned repos under /var/lib/postlab, root-managed known_hosts/.gitconfig
    User(u32),   // project browser: user-owned repos, the invoking user's identity (replaces today's `su`)
}

pub enum GitCreds {
    None,
    HttpsToken { host: String, token: String },  // written to a 0600 cred file/helper — NEVER embedded in the URL
    SshKey { private_key_path: PathBuf },
}

pub enum PullResult {
    FastForwarded { from: String, to: String },
    AlreadyUpToDate,
}

/// Status of the git *binary* (not a repo). Named to avoid colliding with the existing
/// repo-status `GitStatus` in core/projects/mod.rs.
pub struct GitInstall {
    pub installed: bool,
    pub version: String,
}
```

### Operations

Public operations return `Result<_, GitError>` (typed — see [Error handling](#error-handling)),
not bare `anyhow`, so callers and the deploy WebSocket get actionable variants.

```rust
impl GitRepo {
    pub fn new(path: PathBuf, remote: String, creds: GitCreds, run_as: RunAs) -> Self;

    /// Clone `remote` (optionally a single `branch`, optionally shallow) into `path`.
    /// Streams progress lines to `tx`.
    pub async fn clone(&self, branch: Option<&str>, depth: Option<u32>, tx: &Sender) -> Result<(), GitError>;

    /// Fetch then pull --ff-only. Fails GitError::DirtyTree if the tree is dirty unless `force`.
    pub async fn pull_ff_only(&self, force: bool, tx: &Sender) -> Result<PullResult, GitError>;

    /// Fetch then checkout `rev` (branch, tag, or SHA) in detached HEAD.
    /// With `force`, resets and `git clean`s the working tree — SAFE only because generated
    /// artifacts live in ../generated, OUTSIDE this tree (see canonical layout).
    pub async fn checkout(&self, rev: &str, force: bool, tx: &Sender) -> Result<(), GitError>;

    pub async fn current_sha(&self) -> Result<String, GitError>;
    pub async fn is_dirty(&self) -> Result<bool, GitError>;          // tracked modifications
    pub async fn remote_sha(&self, branch: &str) -> Result<String, GitError>;
}
// `Sender` = mpsc::UnboundedSender<String>
```

`has_untracked` from the earlier draft is dropped: `is_dirty` (porcelain) already covers the
case the deploy path cares about, and untracked files in `repo/` are removed by `checkout(force)`.

## Authentication & key management

### Canonical on-disk layout

Generated artifacts (wadm/compose/.env, deploy logs) live **outside** the git working tree, so
a `checkout --force` / `git clean` on rollback can never wipe them. The deploy key sits
**alongside** the repo, not inside `repo/.git`.

```
/var/lib/postlab/
├── .gitconfig                 # GIT_CONFIG_GLOBAL
├── .ssh/known_hosts           # root-managed (RunAs::Root); User repos use the user's ~/.ssh
└── apps/<id>/
    ├── repo/                  # working tree — only git touches this
    ├── deploy_key             # ed25519 private key, mode 0600 (NOT under repo/.git)
    ├── generated/             # wadm.yaml / compose / .env — safe from git clean
    └── deploys/<deploy_id>.log
```

This is the same layout the roadmap's "Canonical on-disk layout" section pins, and it requires
the Phase 0 doc-sync edit to `postlab_go.md` (which currently says `~/postlab`).

### Credentials strategy

| Remote type | Storage | How it is used |
|---|---|---|
| Public HTTPS | none | Plain clone |
| Private HTTPS (token) | per-app mode-0600 credential file (or encrypted DB column) | `git credential` helper / `GIT_CONFIG_GLOBAL` credential store — **never** `https://<token>@host/...`, which leaks the token to `ps`, the reflog, and the persisted `.git/config` |
| Private SSH | per-app ed25519 key file | `GIT_SSH_COMMAND='ssh -i <key> -o StrictHostKeyChecking=accept-new -o BatchMode=yes'` |

The current `core/projects/mod.rs` already does the right thing (writes a `0600` credential
file, not a URL); the wrapper must preserve that, not regress to URL embedding.

### Key files

- Private keys at `/var/lib/postlab/apps/<id>/deploy_key`, mode `0600`, owned by root (for
  `RunAs::Root`). `RunAs::User` repos use the invoking user's `~/.ssh`.
- Public keys can be displayed with a CLI command so the user can add them to GitHub/GitLab.
- Known hosts for root deploys are managed in `/var/lib/postlab/.ssh/known_hosts`, not the root
  user's personal home.

### CLI commands

```bash
postlab git deploy-key --app <id>          # generate or show ed25519 deploy key
postlab git allow-host <host>              # ssh-keyscan host into managed known_hosts
postlab git set-token --app <id>           # store HTTPS token (read from stdin/file, NOT argv)
```

These mirror Dokku's `git:generate-deploy-key`, `git:allow-host`, and `git:auth` commands.
`set-token` reads the secret from stdin or a file, never as an argv parameter (which would leak
to `ps` and shell history). Adding these three subcommands requires a `feature_list.json` update.

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

`AppManager::deploy()` will use the wrapper like this (the whole block runs **inside the per-app
deploy lock** from roadmap Phase 3c — `GitRepo` is not independently concurrency-safe):

```rust
let repo = GitRepo::new(
    app_dir.join("repo"),        // /var/lib/postlab/apps/<id>/repo
    app.repo_url.clone(),
    creds,
    RunAs::Root,
);

if !repo.path.join(".git").exists() {
    repo.clone(Some(&app.repo_branch), Some(1), &tx).await?;   // branch-aware, shallow
} else {
    match repo.pull_ff_only(force, &tx).await? {
        PullResult::AlreadyUpToDate => {}
        PullResult::FastForwarded { from, to } => info!("pulled {}..{}", from, to),
    }
}

let sha = repo.current_sha().await?;
```

Rollback calls `repo.checkout(previous_sha, /*force*/ true, &tx)`; webhook-triggered deploys
pass the pushed SHA to `checkout` when it differs from `HEAD`.

The `core/projects/mod.rs` clone/pull/status logic is refactored to call `GitRepo` with
`RunAs::User(uid)` — **preserving its current user-owned checkout behavior** (it shells out via
`su` today) rather than silently moving the project browser to root-owned files.

## Fresh-Linux integration

The wrapper depends on the prerequisite harness planned in `docs/research/postlab_go_tooling_problems.md`:

- `git` itself is checked by `postlab go --doctor`.
- If `git` is missing, the package adapter for the detected platform installs it (`git` on apt/dnf/pacman).
- `ssh-keygen` is pulled in transitively by the `openssh-client` / `openssh` package on every supported distro.
- Because we shell out to `git`, no extra Rust build dependencies are introduced.

## Error handling

Convert raw git exit statuses into actionable, typed errors. Use `thiserror` so the public
`GitRepo` ops return `Result<_, GitError>` (the rest of the codebase uses `anyhow`, but these
operations need discriminable variants for the UI):

```rust
#[derive(thiserror::Error, Debug)]
pub enum GitError {
    #[error("git is not installed")]                 NotInstalled,
    #[error("working tree has local changes")]       DirtyTree,
    #[error("cannot fast-forward; remote diverged")] FastForwardRejected,
    #[error("authentication failed")]                AuthFailed,
    #[error("host key rejected")]                    HostKeyRejected,
    #[error("remote not found")]                     RemoteNotFound,
    #[error(transparent)]                            Io(#[from] std::io::Error),
}
```

The deploy pipeline can map `GitError` straight onto WS `deploy.progress`/failure events, so the
UI shows "Working tree has local changes" rather than "git pull failed with status 1". Callers
that don't care about variants can still `?`-convert into `anyhow` at the boundary.

## Phases

### Phase 1 — Core wrapper (foundational)

- Create `cli/src/core/git/` with `repo.rs`, `creds.rs`, `error.rs`.
- Implement `GitRepo::new` (**with `run_as` from the start**), `clone` (branch + shallow),
  `pull_ff_only`, `checkout`, `current_sha`, `is_dirty`, `remote_sha`.
- Wire progress streaming through `mpsc::UnboundedSender<String>`; env hardening on every call.
- Add `GitError` (`thiserror`) and return `Result<_, GitError>` from public ops.
- **Delete `core/deploy/git.rs` outright** — it has zero external callers (the only `clone_repo`/
  `pull_repo` references are `projects/mod.rs`'s own same-named methods), so no re-export bridge
  is needed. A re-export would only un-satisfy its `#[expect(dead_code)]` and fail `make check`.

### Phase 2 — Auth & keys

- Add `GitCreds::HttpsToken` (0600 cred file, **not** URL-embedded) and `GitCreds::SshKey`.
- Implement per-app key generation and storage under `/var/lib/postlab/apps/<id>/deploy_key`.
- Implement `postlab git deploy-key`, `allow-host`, and `set-token` (secret via stdin/file).
- Update `feature_list.json` with the three subcommands.

### Phase 3 — Refactor existing callers

- Rewrite `core/projects/mod.rs::{clone_repo, pull_project}` to call `GitRepo` with
  **`RunAs::User(uid)`**, preserving today's user-owned checkout behavior (no ownership
  regression for the TUI project browser, which currently runs git via `su`).
- Keep `git_status` / `set_git_identity` / `set_github_token` as thin shims over
  `core/git/creds.rs`. Rename the wrapper's binary-status type to `GitInstall` to avoid colliding
  with the existing repo-status `GitStatus`.
- Remove duplicated shell-quoting / `GIT_SSH` string logic.

### Phase 4 — `postlab go` integration

> Lands later, in **roadmap Phase 5 (walking skeleton)** — this doc only ships the wrapper
> (roadmap Phase 1). The integration points below are the contract the deploy pipeline
> consumes once it exists; they are listed here so the wrapper's API is designed for them.

- `AppManager::deploy()` constructs a `GitRepo { run_as: RunAs::Root, .. }` pointed at
  `/var/lib/postlab/apps/<id>/repo` and calls `clone`/`pull_ff_only`/`checkout`.
- All git work happens **inside the per-app deploy lock** (roadmap Phase 3 concurrency
  primitive) so two deploys of the same app never touch the same working tree concurrently.
- Rollback uses `GitRepo::checkout` to the previously recorded SHA.
- Deploy log records the resolved `current_sha()` after checkout.
- Webhook-triggered deploys pass the pushed SHA to `checkout` when it differs from `HEAD`.
- Generated artifacts (`wadm.yaml`, `.env`, compose files) are written to
  `/var/lib/postlab/apps/<id>/generated/`, **outside** `repo/`, so a future `git clean`
  cannot destroy them.

### Phase 5 — Tooling & doctor

> Lands in **roadmap Phase 4 (prerequisite harness)**. The wrapper exposes the install
> probe; the harness consumes it.

- Register `git` as a `ToolRequirement` in `core/tooling/` (the harness from
  `docs/research/postlab_go_tooling_problems.md`). Its `check()` calls
  `GitRepo::install_status() -> GitInstall`.
- `postlab go --doctor` and `GET /api/v1/health/tools` report git presence + version.
- `ensure()` installs `git` via `PackageManager::install_many()` if missing.

## Testing

- Unit tests with a local bare repo created in `tempfile::tempdir()` (all `RunAs::Root`,
  since CI runs as the test user against a temp dir it owns):
  - clone into empty dir; clone with `branch`/`depth` set
  - `pull_ff_only` succeeds on fast-forward
  - `pull_ff_only` returns `GitError::FastForwardRejected` on divergent history
  - `pull_ff_only` returns `GitError::DirtyTree` on a dirty working tree
  - `checkout` to previous commit and back; `current_sha()` matches
  - `install_status()` returns `GitInstall { installed: true, .. }`
- Credential safety test: with `GitCreds::HttpsToken`, assert the spawned `git` argv and the
  written `.git/config` contain no token substring (token lives only in the 0600 helper file).
- Integration test on a fresh Ubuntu VM:
  - install postlab binary
  - run `postlab git deploy-key --app test`
  - clone a private GitHub repo using the deploy key
  - pull a new commit and verify `current_sha()`

## Resolved decisions

These were open questions in the first draft; the rewrite settles them so implementation
doesn't re-litigate:

1. **Ownership — resolved via `RunAs`.** Rather than picking root *or* user globally, the
   `run_as: RunAs::{Root, User(uid)}` field is baked into `GitRepo` from day one.
   `postlab go` deploys use `RunAs::Root` against `/var/lib/postlab/apps/*`; the TUI project
   browser keeps `RunAs::User(uid)` so it preserves today's user-owned checkout behavior
   (no ownership regression). One wrapper, two callers, no separate user-mode type.
2. **Canonical path — resolved to `/var/lib/postlab`.** FHS-correct, root-owned. The
   `postlab_go.md` design doc's `~/postlab/apps/<id>/` reference is reconciled to this in
   roadmap Phase 0 (doc-sync). Per-app layout: `repo/`, `deploy_key` (0600, alongside
   `repo/`, **not** inside `.git`), `generated/`, `deploys/<id>.log`.
3. **Token storage — resolved: never in the remote URL.** HTTPS tokens go to a mode-0600
   credential file / `credential.helper`, never `https://<token>@host/...` (which leaks to
   `ps`, shell history, and `.git/config`). This matches the repo's existing secret-handling
   pattern from `core/projects/mod.rs` (which already writes `~/.git-credentials` at 0600).

## Still open

1. **SSH host key policy.** `StrictHostKeyChecking=accept-new` is convenient but allows
   first-connect MITM. A stricter pin-on-first-add mode could be offered later via the
   per-app `known_hosts`.
2. **LFS and submodules.** Out of scope for v1. If needed, add `fetch --all` and
   `submodule update --init --recursive` behind explicit flags.

## Success criteria

- `make check && make test` pass with zero warnings.
- `core/deploy/git.rs` no longer contains dead stubs.
- `core/projects/mod.rs` no longer shells out to `git` directly (calls `GitRepo` with
  `RunAs::User`).
- No git remote URL ever embeds a token; `ps aux` during a deploy shows no secret.
- `postlab go` deploys from a private repo using either HTTPS token or SSH deploy key on a
  fresh Linux VM, with generated artifacts surviving a `git clean` in `repo/`.
