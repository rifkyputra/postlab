# postlab go — Web Orchestration Layer

## Overview

`postlab go` starts a web server on localhost that provides a browser-based UI for application orchestration. It complements the TUI: the TUI handles system-level server management (packages, security, firewall, users, storage), while the web UI handles application-level orchestration (deploy, logs, env vars, domains, templates).

```
┌─ postlab TUI (screens 1-9) ──────────────────────────┐
│  System-level: security, packages, services,          │
│  firewall, users, storage, processes, hardware        │
│  → Keyboard-driven, works over SSH, live gauges       │
├─ postlab web UI (postlab go) ────────────────────────┤
│  App-level: deploy, logs, env vars, domains,          │
│  templates, databases, metrics                        │
│  → Browser-based, real-time streaming, visual         │
├───────────────────────────────────────────────────────┤
│  Shared: SQLite DB pool, core modules (git, docker,   │
│  caddy, systemd, platform adapters), deploy runner    │
└───────────────────────────────────────────────────────┘
```

## Philosophy

Postlab democratizes Linux configuration. The web UI generates real, inspectable, user-editable config files on disk — never hidden behind an opaque platform abstraction. Every deploy writes artifacts the user can `cat`, `vim`, `git diff`, or delete. The orchestration is a speed layer, not a lock-in.

Generated configs follow one rule: **postlab never overwrites user edits without asking**. On redeploy, it diffs the generated files and prompts before overwriting.

## Runtime model

`postlab go` starts the web server inline — same binary, same tokio runtime, same SQLite pool. The TUI and web server are mutually exclusive: you run one or the other.

```bash
$ sudo postlab go
  ▸ API + UI     http://127.0.0.1:9020   (token: sk-postlab-… — printed once)
  ▸ Metrics      http://127.0.0.1:9022

  Press Ctrl+C to stop.
```

Ports are configurable: `--port`, `--webhook-port`, `--metrics-port`. By default only the API/UI and metrics start, both bound to `127.0.0.1`. The webhook receiver is **opt-in** (`--webhook`) because it binds `0.0.0.0` and triggers root-privileged deploys from the public internet — it should only run when the user explicitly wants push-to-deploy. An optional `--bind 0.0.0.0` flag exposes the API/UI to the network.

### Authentication is always on

Even on `127.0.0.1`, the API requires a Bearer token. Localhost is not a trust boundary: on a multi-user host any local process can reach the port, and a browser UI is reachable via DNS-rebinding/CSRF from any page the admin visits. Because every endpoint can clone arbitrary repos, write `/etc/systemd/system/*.service`, and run docker as root, the token is mandatory in all bind modes. See [Security](#security).

### Why not spawn a child process?

A child process adds IPC complexity, crash isolation overhead, and a separate release cycle — none of which is needed for v1. The web server is lightweight (axum on tokio). If the machine has resources for a TUI, it has resources for this.

## CLI

```bash
postlab go                           # Start API/UI + metrics on defaults (9020/9022)
postlab go --port 8080               # Custom API port
postlab go --metrics-port 9090       # Custom metrics port
postlab go --webhook                 # Also start the webhook receiver (0.0.0.0:9021)
postlab go --webhook-port 9000       # Custom webhook port (implies --webhook)
postlab go --bind 0.0.0.0            # Expose API to network (token still required)
```

The API token is generated and printed on first run, then persisted (hashed) in the DB. To supply your own, set `POSTLAB_API_KEY` in the environment or `--api-key-file <path>` — never `--api-key <secret>` on argv, which leaks the secret to `ps` and shell history.

## Architecture (`cli/src/web/`)

```
cli/src/web/
├── mod.rs              # Server startup, port bind, graceful shutdown
├── state.rs            # AppState (SqlitePool + Platform + AppManager)
├── routes/
│   ├── mod.rs          # Router construction (axum::Router)
│   ├── apps.rs         # CRUD + deploy/stop/start/rollback
│   ├── env.rs          # Env var management
│   ├── deploys.rs      # Deploy history + log retrieval
│   ├── templates.rs    # Template catalog + deploy
│   ├── webhooks.rs     # GitHub/GitLab webhook handlers
│   ├── domains.rs      # Caddy route status + TLS certs
│   └── metrics.rs      # Prometheus scrape endpoint
├── ws.rs               # WebSocket for live deploy progress + status
└── assets.rs           # rust-embed static file serving
```

### Shared state

```rust
pub struct AppState {
    pub db: SqlitePool,
    pub platform: Arc<Platform>,
    pub app_manager: Arc<AppManager>,
    pub ws_registry: Arc<WsRegistry>,
}
```

`AppManager` is a new `core/` module that orchestrates deploy workflows. It builds on `core/gateway/` (the `CaddyManager` — `add_route`/`remove_route`/`reload` are already implemented and usable as-is), `core/docker/`, and `core/services/`.

**Caveat on `core/deploy/`.** Treat `git.rs`, `runner.rs`, and `detector.rs` as skeleton, not foundation. Today they are dead-code stubs (`#[expect(dead_code)]`): `detector.rs` recognizes only docker-compose and wasmcloud, `git.rs::pull_repo` runs a bare `git pull` (not the `--ff-only` the workflow requires), and `runner.rs` only handles `DockerCompose`/`WasmCloud`/`Unknown`. The detector table, the runtime backends, and the `--ff-only` git layer in this plan are effectively **net-new code** that may replace those stubs — they are not pre-existing machinery to wire up. Phase estimates below reflect this.

## Data model

### Schema mechanism

The repo does **not** apply the `migrations/*.sql` directory at runtime — there is no `sqlx::migrate!` call anywhere, and the live schema is created with inline `CREATE TABLE IF NOT EXISTS` statements in `cli/src/db/mod.rs`. To stay consistent, the `apps`/`app_env_vars`/`app_deploys` tables below are added the same way: as `CREATE TABLE IF NOT EXISTS` blocks in `db/mod.rs` (or a `db/apps.rs` helper it calls), **not** as new files under `migrations/`. The SQL below is the schema definition, not a migration artifact. (Wiring up `sqlx::migrate!` is a separate, repo-wide decision and out of scope for `postlab go`.)

### `apps` table

```sql
CREATE TABLE IF NOT EXISTS apps (
    id             TEXT PRIMARY KEY,       -- slug: "my-api"
    name           TEXT NOT NULL,
    runtime        TEXT NOT NULL,          -- HOW it runs: docker-compose | k3s | pm2 | systemd | wasmcloud | static
    language       TEXT,                   -- WHAT it is (informational): node | python | go | rust | static
    repo_url       TEXT NOT NULL,
    repo_branch    TEXT NOT NULL DEFAULT 'main',
    domain         TEXT,                   -- api.example.com (null if no public domain)
    port           INTEGER NOT NULL,       -- internal port the app listens on
    health_path    TEXT NOT NULL DEFAULT '/',
    status         TEXT NOT NULL DEFAULT 'created',
    webhook_secret TEXT NOT NULL,          -- per-app HMAC secret (required; see Webhooks)
    config_dir     TEXT NOT NULL,          -- ~/postlab/apps/<id>/
    created_at     TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at     TEXT NOT NULL DEFAULT (datetime('now'))
);
```

status values: `created` | `deploying` | `running` | `stopped` | `failed` | `rolling_back`

**`runtime` vs `language`.** `runtime` is the single source of truth for *how* the app is started/stopped/logged — it selects the `RuntimeBackend`. `language` is a purely informational tag the detector records (used only for UI badges and to pick a build template); it never drives dispatch. This replaces the earlier overloaded `deploy_type` column and avoids the three-way overlap with the legacy `core::models::DeploymentType` enum, which `postlab go` does not use.

### `app_env_vars` table

```sql
CREATE TABLE IF NOT EXISTS app_env_vars (
    app_id   TEXT NOT NULL REFERENCES apps(id) ON DELETE CASCADE,
    key      TEXT NOT NULL,
    value    TEXT NOT NULL,
    secret   INTEGER NOT NULL DEFAULT 1,   -- mask in UI
    PRIMARY KEY (app_id, key)
);
```

### `app_deploys` table

```sql
CREATE TABLE IF NOT EXISTS app_deploys (
    id            TEXT PRIMARY KEY,        -- uuid v7 (time-ordered; uuid crate, add the "v7" feature)
    app_id        TEXT NOT NULL REFERENCES apps(id) ON DELETE CASCADE,
    commit_sha    TEXT,
    commit_msg    TEXT,
    status        TEXT NOT NULL,           -- pending | building | deploying | running | failed | rolled_back
    started_at    TEXT NOT NULL DEFAULT (datetime('now')),
    finished_at   TEXT,
    log_ref       TEXT                     -- path to deploy log file on disk
);
```

Logs are written to `~/postlab/apps/<app_id>/deploys/<deploy_id>.log` on disk. The DB stores the path, not the log content. This keeps the SQLite database small and allows streaming large logs.

### App metrics — in-memory, not a table

App metrics are **not** persisted to SQLite. A 5-second sample per app for a 1-hour window is 720 rows/app of pure write amplification on a WAL database, and the original `PRIMARY KEY (app_id, timestamp)` would collide whenever two samples land in the same wall-clock second (`datetime('now')` is 1-second granularity).

Instead each app gets a fixed-size in-memory ring buffer (720 slots) on `AppManager`:

```rust
struct MetricSample { ts: i64, cpu_percent: f32, mem_bytes: u64, req_count: u64 }
// AppManager: DashMap<String, ArrayDeque<MetricSample, 720>>
```

A periodic sampler task pushes one sample per app every 5s; the buffer overwrites the oldest slot. The `/metrics` endpoint and the UI sparklines read straight from memory. Samples are lost on restart, which is fine for a live-monitoring view.

## API design

All endpoints under `/api/v1`. JSON request/response bodies.

### Apps

```
GET    /api/v1/apps                    List all apps (id, name, runtime, status, domain, updated_at)
POST   /api/v1/apps                    Create app → returns app with status 'created'
GET    /api/v1/apps/:id                App detail with latest deploy summary
DELETE /api/v1/apps/:id                Delete app + cascade (env vars, deploys, metrics, config files)
POST   /api/v1/apps/:id/deploy         Redeploy (git pull → build → restart → health check)
POST   /api/v1/apps/:id/rollback       Rollback to previous successful deploy
POST   /api/v1/apps/:id/stop           Stop app (runtime-specific)
POST   /api/v1/apps/:id/start          Start app (runtime-specific)
```

### Env vars

```
GET    /api/v1/apps/:id/env            List env vars (values masked for secrets)
PUT    /api/v1/apps/:id/env            Bulk upsert env vars { "vars": { "KEY": "value", ... } }
DELETE /api/v1/apps/:id/env/:key       Delete a single var
```

### Deploys

```
GET    /api/v1/apps/:id/deploys        Deploy history (paginated)
GET    /api/v1/apps/:id/deploys/:did   Single deploy detail + metadata
GET    /api/v1/apps/:id/deploys/:did/log  Raw deploy log (streamed as text/plain)
```

### Templates

```
GET    /api/v1/templates               List available templates
POST   /api/v1/templates/:id/deploy    Deploy a template → returns created app
```

Templates are a static catalog defined in `cli/src/web/templates.json` (embedded at compile time). Each template specifies runtime, repo URL or docker image, default port, suggested env vars, and health path.

### Webhooks

```
POST   /api/v1/webhooks/github         GitHub push webhook
POST   /api/v1/webhooks/gitlab         GitLab push webhook
```

Payloads are validated against the webhook secret (configured per app). The webhook identifies the target app by matching the repo URL in the payload against registered apps.

### Domains

```
GET    /api/v1/domains                 List Caddy routes with TLS status + cert expiry
```

### WebSocket

```
WS     /ws                             Live event stream
```

Events are JSON messages with a `type` field:

```json
{"type":"deploy.progress","app_id":"x","step":"building","ts":"...","output":"npm ci..."}
{"type":"deploy.progress","app_id":"x","step":"health_check","ts":"...","output":"GET / 200 OK"}
{"type":"deploy.complete","app_id":"x","status":"running","deploy_id":"d1"}
{"type":"app.status","app_id":"x","status":"stopped"}
{"type":"app.log","app_id":"x","stream":"stdout","ts":"...","text":"listening on :3000"}
```

The WS registry (`WsRegistry`) holds a `DashMap<String, Vec<Sender>>` keyed by app_id. When a deploy runs, it broadcasts progress events to all connected clients viewing that app.

### Metrics

```
GET    /metrics                        Prometheus text format (port 9022)
```

Exposes app-level metrics (deploy count, uptime, runtime status) and system-level metrics (CPU, memory, disk) pulled from the Platform.

## Frontend

Built with **SvelteKit + `adapter-static` in SPA mode** (`fallback: index.html`, all routes prerendered off). The file-based `routes/` tree below is SvelteKit's convention; SPA mode produces a purely static bundle with no Node server, which is what gets embedded. (The earlier "Svelte 5 + plain Vite" wording was inconsistent with the `routes/+page.svelte` layout — this is the single intended setup.) Compiled to static JS/CSS/HTML, embedded in the binary via `rust-embed` at build time. The frontend source lives in `web/` at the repo root.

```
web/
├── package.json
├── vite.config.ts
├── src/
│   ├── app.html                # HTML shell
│   ├── app.ts                  # Entry point + router init
│   ├── lib/
│   │   ├── api.ts              # Typed fetch wrappers for all endpoints
│   │   ├── ws.ts               # WebSocket client (reconnect, event dispatch)
│   │   └── stores.ts           # Svelte stores (apps, selected app, events)
│   ├── routes/
│   │   ├── +page.svelte        # Dashboard (app list + system overview)
│   │   ├── apps/
│   │   │   ├── [id]/
│   │   │   │   ├── +page.svelte        # App detail (overview tab)
│   │   │   │   ├── logs/+page.svelte   # Log viewer
│   │   │   │   ├── env/+page.svelte    # Env var editor
│   │   │   │   └── deploys/+page.svelte # Deploy history
│   │   │   └── new/+page.svelte # New app wizard
│   │   └── templates/+page.svelte # Template catalog
│   └── components/
│       ├── AppCard.svelte       # App list item
│       ├── DeployLog.svelte     # Streaming deploy output
│       ├── LogViewer.svelte     # Live log tail
│       ├── EnvEditor.svelte     # Key-value editor with masking
│       ├── RuntimePicker.svelte # Visual runtime selection cards
│       ├── DomainStatus.svelte  # TLS cert status badge
│       ├── MetricsSpark.svelte  # CPU/mem/req sparklines
│       └── StatusBadge.svelte   # Colored status dot
└── static/                      # Favicon, fonts
```

### Build integration

`make build-release` runs `npm ci && npm run build` before `cargo build --release`. The compiled output in `web/dist/` is embedded via:

```rust
#[derive(RustEmbed)]
#[folder = "web/dist/"]
struct WebAssets;
```

At runtime, axum serves the embedded bytes — no npm/node required on the target machine.

**`web/dist/` must always exist or `cargo build` fails.** `rust-embed` resolves `#[folder = …]` at compile time and errors if the directory is missing — which would break `make check`/`make build` (the CLAUDE.md gate) on any checkout that hasn't run the frontend build. Two safeguards:

1. Commit a minimal placeholder `web/dist/index.html` ("run `npm run build`") so the embed always resolves.
2. Enable `rust-embed`'s `debug-embed` feature so dev/`check` builds read assets from disk and only release builds embed them, keeping the placeholder out of shipped binaries.

`make build` and `make check` must not depend on `npm`; only `make build-release` runs the frontend build.

### SPA routing

All `/ui/*` paths serve `index.html`. The Svelte router handles client-side navigation. API calls are relative to `/api/v1/`. No server-side templating — the frontend is purely static.

## Runtime backends

Each runtime backend is a trait implementation in `core/deploy/backends/`. The `AppManager` dispatches to the appropriate backend based on `app.runtime`.

```rust
#[async_trait]
pub trait RuntimeBackend: Send + Sync {
    fn name(&self) -> &'static str;
    async fn generate_config(&self, app: &App, env_vars: &HashMap<String, String>) -> Result<()>;
    async fn start(&self, app: &App) -> Result<()>;
    async fn stop(&self, app: &App) -> Result<()>;
    async fn status(&self, app: &App) -> Result<RuntimeStatus>;
    async fn logs(&self, app: &App, lines: usize) -> Result<Vec<LogLine>>;
    async fn metrics(&self, app: &App) -> Result<RuntimeMetrics>;
    async fn cleanup(&self, app: &App) -> Result<()>;
}
```

### docker-compose

Generates a `Dockerfile` if the repo doesn't have one (buildpack-style auto-detection). Writes `docker-compose.yml` with port mapping and env file. Start/stop via `docker compose up -d` / `docker compose down`. Logs via `docker compose logs --tail N`.

### pm2

Generates `ecosystem.config.js` with script, args, env, memory limits. Start/stop via `pm2 start ecosystem.config.js` / `pm2 stop <app_id>`. Logs via `pm2 logs <app_id> --lines N`. Checks if PM2 is installed; offers to install if absent.

### systemd

Generates a `.service` unit file with `ExecStart`, `WorkingDirectory`, `EnvironmentFile`. Installs to `/etc/systemd/system/<app_id>.service`. Start/stop via `systemctl`. Logs via `journalctl -u <app_id> --no-pager -n N`.

### k3s

Assumes k3s is already installed and configured. Generates `deployment.yaml` + `service.yaml` + `ingress.yaml`. Applies via `kubectl apply -f <dir>/`. Logs via `kubectl logs deployment/<app_id> --tail N`.

### wasmcloud

Existing path from `core/deploy/runner.rs`. Generates `wadm.yaml` if needed. Deploys via `wash app deploy`.

### static

No runtime process. Generates Caddy config pointing to the repo directory. Serves static files directly. No health check, no metrics — just a domain pointing at a directory.

### Detector (auto-select runtime)

`core/deploy/detector.rs` scans the repo after clone and picks a runtime:

| Detection | Runtime |
|-----------|---------|
| `docker-compose.yml` / `compose.yaml` | docker-compose |
| `Dockerfile` (without compose file) | docker-compose (generate minimal compose) |
| `package.json` | pm2 |
| `requirements.txt` / `pyproject.toml` | docker-compose (generate Python Dockerfile) |
| `go.mod` | systemd (build binary) or docker-compose |
| `Cargo.toml` | systemd (build binary) or docker-compose |
| `wadm.yaml` / `wasmcloud.toml` | wasmcloud |
| `index.html` (top-level, no package.json) | static |
| None matched | Prompt user in wizard to choose manually |

## Deploy workflow

```
┌──────────┐    ┌──────────┐    ┌──────────┐    ┌──────────┐    ┌──────────┐
│ git pull │ →  │  build   │ →  │  start   │ →  │ health   │ →  │ gateway  │
│ (clone   │    │ (runtime │    │ (runtime │    │ check    │    │ update   │
│  if new) │    │  detect  │    │  start)  │    │ (poll)   │    │ (Caddy)  │
│          │    │ + build) │    │          │    │          │    │          │
└──────────┘    └──────────┘    └──────────┘    └──────────┘    └──────────┘
     │               │               │               │               │
     │  WS event     │  WS event     │  WS event     │  WS event     │  WS event
     │  "cloning"    │  "building"   │  "starting"   │  "health"     │  "complete"
```

1. **git pull/clone** — If the config dir has no repo, `git clone`. Otherwise `git pull --ff-only` (reject if dirty; user must resolve manually or force-redeploy).

2. **Detect** — If `app.deploy_type` is null, run the detector. If the user manually set a runtime, skip detection.

3. **Build** — Runtime-specific build step. Emits stdout/stderr line-by-line as WS `deploy.progress` events.

4. **Generate config** — Write/update generated config files to `app.config_dir`.

5. **Start** — Runtime-specific start command. If a previous version is running, stop it first (unless zero-downtime is supported — see below).

6. **Health check** — For runtimes that serve HTTP, poll `GET :port/{health_path}` every 500ms, timeout after 30s. On success → step 7; on failure → mark deploy failed, leave old version running. The check is **per-runtime opt-out**, not universal: `static` has no process to probe, and non-HTTP services (raw TCP, queue workers) fall back to a readiness signal from the backend (`RuntimeStatus`) instead of an HTTP GET. Each `RuntimeBackend` declares whether it supports HTTP health checks.

7. **Gateway update** — If `app.domain` is set, add/update the Caddy route pointing to `localhost:{port}`. Caddy auto-TLS provisions the certificate.

8. **Complete** — Update `app.status = 'running'`, write deploy log to disk, emit WS `deploy.complete`.

### Zero-downtime deploys (docker-compose and k3s only)

Instead of stop → start, the backend starts the new version on a temporary port, health-checks it, then atomically swaps the Caddy route before stopping the old version. Systemd and PM2 backends do a simple restart (sub-second downtime). Static sites have zero downtime (just git pull).

### Rollback

Rollback restarts the app using the git tree from the previous successful deploy. It does not git-revert — it just checks out the prior commit and runs the deploy workflow from step 3 (build → start → health → gateway).

## Webhook receiver

The webhook receiver is a separate axum server on port 9021 that binds `0.0.0.0`. It is **opt-in** (`--webhook` / `--webhook-port`) and off by default, because it exposes a root-privileged deploy trigger to the public internet.

There is no shared secret — each app authenticates with its own `apps.webhook_secret`. Because the receiver can't know which app a request targets before validating it, matching happens by candidate set:

When a push webhook arrives:
1. Identify candidate apps by the payload's repo URL (and branch).
2. **Validate the HMAC signature against each candidate's own secret**; reject (401) if none match. Repo URL alone never authorizes a deploy — it only narrows the candidate set, since payload contents are attacker-controllable.
3. **Replay protection.** Reject deliveries whose timestamp is outside a short skew window, and de-dupe on the provider's delivery ID (`X-GitHub-Delivery` / GitLab equivalent) so a captured request can't be replayed.
4. Spawn a `tokio::spawn` deploy task for each authenticated app (non-blocking — returns 202 immediately).
5. The deploy task emits WS events that the frontend picks up.

GitHub payloads deliver commit SHA and message, which are written to `app_deploys`.

## One-click templates

Templates are defined in a compile-time embedded JSON file:

```json
[
  {
    "id": "n8n",
    "name": "n8n",
    "description": "Fair-code workflow automation platform",
    "category": "automation",
    "icon": "n8n",
    "runtime": "docker-compose",
    "repo_url": null,
    "docker_image": "n8nio/n8n:latest",
    "port": 5678,
    "health_path": "/healthz",
    "env_vars": {
      "N8N_HOST": "{domain}",
      "N8N_PORT": "5678",
      "N8N_PROTOCOL": "https",
      "WEBHOOK_URL": "https://{domain}/"
    },
    "volumes": ["n8n_data:/home/node/.n8n"]
  }
]
```

`{domain}` is replaced with the user-supplied domain in the wizard. The template deploy flow is identical to the app wizard but with most fields pre-filled and locked. The user only supplies a name and domain.

Initial templates: n8n, Uptime Kuma, NocoDB, Ghost, MinIO, Plausible, Nextcloud, WordPress.

## Security

### Authentication

A Bearer token is required on every API/UI request **in all bind modes, including `127.0.0.1`**:

```
Authorization: Bearer sk-postlab-...
```

Localhost-only is not a security boundary for a root-privileged deploy API. Two distinct threats make an unauthenticated localhost listener unsafe:

- **Local multi-user / multi-process.** Any user or process that can open `127.0.0.1:9020` gets root-equivalent RCE, because the API clones arbitrary repos, writes systemd units, and runs docker as root.
- **Browser DNS-rebinding / CSRF.** The UI runs in a browser; any web page the admin visits can issue cross-origin requests to `127.0.0.1:9020`. A token the page can't read, plus the Origin check below, closes this.

Defenses, always on:

1. **Mandatory Bearer token.** Generated on first run, printed once to stdout, stored **hashed** in the DB (never logged). Supplied by the user via `POSTLAB_API_KEY` or `--api-key-file <path>` — never as an argv flag (leaks to `ps`/history). The static frontend receives the token via the login flow and holds it in memory only.
2. **Origin / Host allow-list.** Reject requests whose `Origin` or `Host` header isn't an expected loopback/configured value. This blocks DNS-rebinding even when the token is somehow present.

The webhook receiver uses per-app HMAC signature validation, not Bearer tokens (see [Webhook receiver](#webhook-receiver)).

### Running as root

The web server runs as root (same as the TUI). This is required because it spawns docker, systemctl, caddy, and other privileged commands. The frontend can only invoke operations through the API, which validates inputs and escapes shell arguments.

### Webhook secrets

Each app has its **own** HMAC secret (`apps.webhook_secret`), generated when the app is created. There is **no** global/shared fallback secret: a single leaked secret must not be able to trigger deploys for other apps. Server-level settings that do need persisting (e.g. whether the receiver is enabled) reuse the existing `projects_config` key/value table — there is no `config` table in this codebase. Each app's secret is shown once in the UI for the user to paste into their GitHub/GitLab webhook settings.

## Port conflict resolution

On startup, `postlab go` checks each port. If a default port is in use:

1. Try the next available port, **skipping the other reserved defaults** (9020, 9021, 9022). The API must not fall back onto 9021/9022 and stomp the webhook/metrics services — probe upward from a per-service base into a non-overlapping band (e.g. API 9020→9023→9024…, webhook 9021→9031…, metrics 9022→9041…).
2. Print a warning: "Port 9020 in use, using 9023 instead".
3. If a port was set explicitly (`--port`/`--webhook-port`/`--metrics-port`) and is in use, error out immediately with a clear message — never silently move an explicitly chosen port.

## Graceful shutdown

On SIGINT/SIGTERM:
1. Close the WebSocket connections with a close frame
2. Drain in-flight deploy tasks (wait up to 5s)
3. Drop the axum server
4. Flush SQLite WAL
5. Exit 0

Running apps are not stopped — they continue running independently. Postlab is an orchestrator, not a runtime supervisor.

## Implementation phases

### Phase 1 — Scaffold

- [ ] Add `axum`, `tower`, `tower-http`, `rust-embed`, `dashmap` to `cli/Cargo.toml` (versions already pinned in `workspace.dependencies`; not yet consumed by the crate)
- [ ] New `cli/src/web/` module tree
- [ ] `postlab go` CLI subcommand in `main.rs` (`--port`/`--metrics-port`/`--bind`/`--webhook`/`--api-key-file`)
- [ ] `AppState` with `SqlitePool` reference
- [ ] axum router with health check endpoint + WS placeholder
- [ ] **Auth middleware from day one**: mandatory Bearer token (generated, hashed in DB) + Origin/Host allow-list — not deferred to a later phase
- [ ] Committed placeholder `web/dist/index.html` + `rust-embed` `debug-embed` feature so `make check`/`make build` work without npm
- [ ] Port bind with reserved-port-skipping fallback + graceful shutdown
- [ ] `make check` passes cleanly (no npm dependency)

### Phase 2 — Core API + DB

- [ ] `apps` / `app_env_vars` / `app_deploys` schema as inline `CREATE TABLE IF NOT EXISTS` in `db/mod.rs` (matching the existing pattern — **not** files under `migrations/`)
- [ ] `core/apps/` module (CRUD, deploy, stop, start, rollback)
- [ ] **Net-new** `--ff-only` git clone/pull layer (the existing `core/deploy/git.rs` stub uses a bare `git pull`)
- [ ] `AppManager` that drives git → detect → backend → gateway, including the per-app metrics ring buffer
- [ ] REST API: apps CRUD + deploy + stop + start
- [ ] Env var endpoints
- [ ] `POST /api/v1/apps/:id/deploy` wired to git clone → detector → backend → gateway
- [ ] `make check && make test` pass cleanly

### Phase 3 — Frontend

- [ ] `web/` Svelte project scaffolded
- [ ] Dashboard: app list + system overview gauges
- [ ] App detail: overview tab (status, domain, latest deploy)
- [ ] New app wizard (multi-step: runtime, source, domain, env vars, review)
- [ ] Env var editor
- [ ] Deploy history list
- [ ] `npm run build` integration in `make build-release`
- [ ] `rust-embed` serving compiled frontend

### Phase 4 — Live features

- [ ] WebSocket event stream (deploy progress + status changes)
- [ ] Streaming deploy log in the wizard and detail page
- [ ] Live log viewer per app (runtime logs via WS)

### Phase 5 — Templates + webhooks

- [ ] Template catalog (static JSON)
- [ ] `POST /api/v1/templates/:id/deploy`
- [ ] Templates page in frontend
- [ ] Opt-in webhook receiver (`--webhook`, port 9021)
- [ ] Per-app HMAC validation (no shared secret) + candidate matching + replay/delivery-id protection
- [ ] Metrics endpoint (port 9022) reading the in-memory ring buffer

### Phase 6 — Runtime backends (mostly net-new)

The existing `core/deploy/{detector,runner}.rs` are stubs (docker-compose + wasmcloud only); this phase largely replaces them.

- [ ] `RuntimeBackend` trait + dispatch, including the `supports_http_health()` opt-out
- [ ] Detector rewrite (Node, Python, Go, Rust, static — replaces the 2-case stub)
- [ ] PM2 backend
- [ ] systemd backend
- [ ] k3s backend
- [ ] static site backend
- [ ] Zero-downtime deploy (docker-compose, k3s)

## Future: remote access with built-in Tailscale Funnel

Once the web server is stable, `postlab go --funnel` could use Tailscale Funnel to expose the UI securely over the internet without opening firewall ports. This would make the web UI accessible from anywhere with Tailscale's end-to-end encryption.

## Future: WASM plugins for the web UI

The frontend serves as a platform for WASM-based extensions. Third-party developers could write plugins that add new pages to the web UI (e.g., a Grafana dashboard, a Kubernetes visualizer, a database browser). Postlab would expose a JavaScript API for the plugin to call the REST endpoints.

## Future: `postlab go` daemon mode

`postlab go --daemon` would daemonize the web server (systemd integration, PID file, log to journald). This would make the web UI always available, not just when a user runs the command manually. The TUI could then connect to the daemon's API for app-level operations, unblocking both interfaces from running simultaneously.
