# postlab go — Edge-case Analysis

Source: `docs/plan/postlab_go.md`

## 1. Must resolve before scaffolding (HIGH severity)

These issues will block compilation, break security guarantees, or corrupt the deploy workflow if not fixed in the architecture before Phase 2.

| # | Edge case | Why it matters | Recommended fix |
|---|---|---|---|
| 1.1 | **Token hashing scheme is unspecified** | A standard password hash cannot verify a random bearer token because the raw token is discarded after first print. | Store a deterministic hash such as `SHA-256(token)` or `HMAC-SHA256(token, server_secret)`; verify by hashing the submitted token and comparing. |
| 1.2 | **Webhook replay/dedup is not persisted** | In-memory delivery-ID sets are lost on restart, allowing replay attacks after a crash or upgrade. | Persist delivery IDs in SQLite with a TTL (e.g., 24 h) and reject duplicates across restarts. |
| 1.3 | **Candidate matching by repo URL is attacker-controlled** | The payload repo URL narrows candidate apps, but the URL text is supplied by the sender. | Repo URL only narrows candidates; HMAC must be validated against **every** candidate's secret before any deploy triggers. |
| 1.4 | **Existing `git.rs` uses bare `git pull`, not `--ff-only`** | The plan requires `--ff-only`; the current stub does not. | Rewrite `pull_repo` to use `--ff-only` and add explicit dirty-tree detection before pull. |
| 1.5 | **No per-app deploy lock** | Concurrent deploys/rollbacks on the same app can corrupt the git tree, config dir, or Caddy state. | Add `DashMap<app_id, Mutex<()>>` (or equivalent) in `AppManager`; acquire before deploy/rollback/start. |
| 1.6 | **Existing detector only knows docker-compose and wasmcloud** | The plan's detector table (Node, Python, Go, Rust, static) is a full rewrite. | Treat detector as net-new in Phase 6 estimates. |
| 1.7 | **Existing `runner.rs` only handles DockerCompose/WasmCloud/Unknown** | PM2, systemd, k3s, and static backends do not exist. | Treat runtime backends as net-new in Phase 6 estimates. |
| 1.8 | **Zero-downtime temp port collision** | Docker-compose/k3s zero-downtime deploys must pick a free host port; if taken, the health check fails. | Probe bind availability before selecting the temporary port. |
| 1.9 | **Failed zero-downtime deploy can leave orphan containers** | If health check passes but the Caddy gateway update fails, the new container must be stopped. | Add cleanup in the gateway-update failure path. |
| 1.10 | **systemd unit name derived from `app_id` without validation** | Unit names cannot contain `/`, `:`, spaces, etc. | Validate `app_id` as `[a-z0-9_-]+` at creation time. |
| 1.11 | **Inline schema in `db/mod.rs` must exactly match the plan** | The repo does not run `migrations/` at runtime. | Add `apps`/`app_env_vars`/`app_deploys` `CREATE TABLE IF NOT EXISTS` blocks (or a new `db/apps.rs` helper) and keep them in sync with the plan. |
| 1.12 | **`uuid` crate lacks the `v7` feature** | The plan requires uuid v7 for `app_deploys.id`. | Add `"v7"` to the workspace `uuid` features. |
| 1.13 | **Ring-buffer crate is not in deps** | The metric ring buffer needs an implementation. | Add `arraydeque` or implement a fixed-size `VecDeque` wrapper. |

## 2. Authentication and security edge cases

- **Origin/Host allow-list on `127.0.0.1`:** Browsers may send `Origin: null` (e.g., from `file://` or sandboxed contexts). Define whether `null` is rejected or treated as loopback.
- **`/metrics` endpoint has no auth:** Bind to `127.0.0.1:9022` by default and document firewall considerations; do not expose it on `0.0.0.0` without explicit opt-in.
- **`POSTLAB_API_KEY` vs persisted hashed token:** Define precedence when both an env-provided key and a stored hash exist. Suggested precedence: env/file > persisted hash.
- **API token printed once to stdout:** Provide a reset/rotate subcommand or an env-override path so a lost token does not require DB surgery.
- **`--api-key-file` trailing newlines:** Trim whitespace before use to avoid confusing "file not found"-style auth failures.
- **Browser token held only in memory:** Page reloads require re-entry; acceptable, but document this in the UI login screen.

## 3. Deploy workflow edge cases

- **Dirty git tree:** `git pull --ff-only` will fail on dirty trees. Explicitly run `git status --porcelain` first and emit a clear UI error or prompt; add a force-redeploy path to discard local changes.
- **Rollback dirty tree:** Rollback checks out an older commit on the existing tree; the same dirty-tree handling applies.
- **Build output with carriage returns:** Streaming output line-by-line may fail for tools that use `\r` progress (e.g., `npm ci`). Consider splitting on `\r` as well as `\n`.
- **Health check timeout too short:** A fixed 30 s timeout may be too short for JVM or large Python containers. Make the timeout configurable per app, with a sensible default.
- **Detector ambiguity:** A repo can match multiple detectors (e.g., `Dockerfile` + `package.json`). Define precedence or prompt the user in the wizard.

## 4. Runtime backend edge cases

- **`static` runtime has no health check:** The backend must opt out via `supports_http_health()` and the UI must not show a failing health indicator.
- **Non-HTTP services:** Queue workers or raw TCP services need a readiness signal definition (process existence, socket bind, or systemd `ActiveState`) instead of HTTP polling.
- **PM2 installation as root:** Offering to install PM2 while running as root is discouraged on most distributions. Prefer OS package manager installation or require explicit user consent.
- **k3s backend prerequisites:** Assumes `kubectl` and a configured cluster. Document prerequisites and fail gracefully if missing.
- **wasmcloud backend conflict:** The existing `runner.rs` wasmcloud path may need to be wrapped or replaced by the new `RuntimeBackend` trait.

## 5. Build and dependency edge cases

- **`web/dist/` must exist at compile time:** `rust-embed` resolves `#[folder = "web/dist/"]` at compile time. Commit a placeholder `index.html` and enable `debug-embed` so `make check`/`make build` do not depend on npm.
- **`rust-embed` compression in dev builds:** The workspace pins `compression` but not `debug-embed`; dev/`check` builds will embed assets unless `debug-embed` is added.
- **Unused workspace deps in `cli/Cargo.toml`:** `axum`, `tower`, `tower-http`, `rust-embed`, and `dashmap` are pinned in the workspace but not yet consumed by `cli/Cargo.toml`; adding them in Phase 1 is required.
