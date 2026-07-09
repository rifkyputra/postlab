# PROGRESS.md

> Cross-session handoff artifact. Updated at the end of every workflow phase.
> A fresh agent session reads this to pick up where the last session stopped.
> Format: "done | in-progress | blocked" per feature.

## Active work

| Feature | Phase | Status | Artifacts | Last updated |
|---|---|---|---|---|
| Phase 1 — Git wrapper rewrite | implementation (1b next) | in-progress | `docs/plan/postlab_git_rewrite.md` | 2026-06-30 |

### Phase 1 remaining sub-phases
- **1b** — `GitCreds::{HttpsToken, SshKey}` auth wiring; `postlab git deploy-key/allow-host/set-token` CLI commands; update `feature_list.json`
- **1c** — Refactor `core/projects/mod.rs` to call `GitRepo` with `RunAs::User(uid)`; delete duplicated shell-quoting / `GIT_SSH` string logic

### Phase 1a — done (2026-06-30)
- `core/git/` created: `repo.rs`, `creds.rs`, `error.rs`
- `GitRepo::{clone, pull_ff_only, checkout, current_sha, is_dirty, remote_sha, install_status}`
- `GitError` (thiserror), `PullResult`, `GitInstall`, `RunAs`, `GitCreds` types
- Progress streaming via `mpsc::UnboundedSender<String>`, env hardening on every call
- `core/deploy/git.rs` deleted
- 9 unit tests (tempdir bare repo): clone, pull ff/dirty/diverged, checkout, is_dirty, install_status
- `make check && make test` green (102 tests)

## Blocked

| Feature | Blocked by | Since |
|---|---|---|

## Recently merged

| Feature | Merged | PR |
|---|---|---|
| Phase 0 — dep/repo prep | 2026-06-30 | — |
