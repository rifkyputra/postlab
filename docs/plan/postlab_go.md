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
  ▸ API + UI     http://127.0.0.1:9020
  ▸ Webhook rcvr http://0.0.0.0:9021
  ▸ Metrics      http://127.0.0.1:9022

  Press Ctrl+C to stop.
```

Ports are configurable: `--port`, `--webhook-port`, `--metrics-port`. The webhook receiver binds to `0.0.0.0` (must receive external traffic from GitHub/GitLab). Everything else defaults to `127.0.0.1`. An optional `--bind 0.0.0.0` flag exposes the API/UI to the network, paired with `--api-key <secret>` for authentication.

### Why not spawn a child process?

A child process adds IPC complexity, crash isolation overhead, and a separate release cycle — none of which is needed for v1. The web server is lightweight (axum on tokio). If the machine has resources for a TUI, it has resources for this.

## CLI

```bash
postlab go                           # Start on defaults (9020/9021/9022)
postlab go --port 8080               # Custom API port
postlab go --webhook-port 9000       # Custom webhook port
postlab go --metrics-port 9090       # Custom metrics port
postlab go --bind 0.0.0.0            # Expose API to network
postlab go --api-key sk-xxx          # Require Bearer token on API
```

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

`AppManager` is a new `core/` module that orchestrates deploy workflows. It calls into existing `core/deploy/git.rs`, `core/deploy/runner.rs`, `core/gateway/`, `core/docker/`, `core/services/`, and new runtime backends.

## Data model

### `apps` table

```sql
CREATE TABLE apps (
    id            TEXT PRIMARY KEY,        -- slug: "my-api"
    name          TEXT NOT NULL,
    runtime       TEXT NOT NULL,           -- docker-compose | k3s | pm2 | systemd | wasmcloud | static
    repo_url      TEXT NOT NULL,
    repo_branch   TEXT NOT NULL DEFAULT 'main',
    domain        TEXT,                    -- api.example.com (null if no public domain)
    port          INTEGER NOT NULL,        -- internal port the app listens on
    health_path   TEXT NOT NULL DEFAULT '/',
    status        TEXT NOT NULL DEFAULT 'created',
    deploy_type   TEXT,                    -- detected strategy (e.g., node-express, rust-bin, go-binary)
    config_dir    TEXT NOT NULL,           -- ~/postlab/apps/<id>/
    created_at    TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at    TEXT NOT NULL DEFAULT (datetime('now'))
);
```

status values: `created` | `deploying` | `running` | `stopped` | `failed` | `rolling_back`

### `app_env_vars` table

```sql
CREATE TABLE app_env_vars (
    app_id   TEXT NOT NULL REFERENCES apps(id) ON DELETE CASCADE,
    key      TEXT NOT NULL,
    value    TEXT NOT NULL,
    secret   INTEGER NOT NULL DEFAULT 1,   -- mask in UI
    PRIMARY KEY (app_id, key)
);
```

### `app_deploys` table

```sql
CREATE TABLE app_deploys (
    id            TEXT PRIMARY KEY,        -- ULID
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

### `app_metrics` table

```sql
CREATE TABLE app_metrics (
    app_id       TEXT NOT NULL REFERENCES apps(id) ON DELETE CASCADE,
    timestamp    TEXT NOT NULL DEFAULT (datetime('now')),
    cpu_percent  REAL,
    mem_bytes    INTEGER,
    req_count    INTEGER,
    PRIMARY KEY (app_id, timestamp)
);
```

Rolling window, pruned by a periodic janitor task. Stores 1 hour at 5-second resolution.

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

Built with Svelte 5 + Vite. Compiled to static JS/CSS/HTML, embedded in the binary via `rust-embed` at build time. The frontend source lives in `web/` at the repo root.

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

`rust-embed` is a **build dependency** only. At runtime, axum serves the embedded bytes — no npm/node required on the target machine.

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

6. **Health check** — Poll `GET :port/{health_path}` every 500ms, timeout after 30s. On success → step 7. On failure → mark deploy failed, leave old version running.

7. **Gateway update** — If `app.domain` is set, add/update the Caddy route pointing to `localhost:{port}`. Caddy auto-TLS provisions the certificate.

8. **Complete** — Update `app.status = 'running'`, write deploy log to disk, emit WS `deploy.complete`.

### Zero-downtime deploys (docker-compose and k3s only)

Instead of stop → start, the backend starts the new version on a temporary port, health-checks it, then atomically swaps the Caddy route before stopping the old version. Systemd and PM2 backends do a simple restart (sub-second downtime). Static sites have zero downtime (just git pull).

### Rollback

Rollback restarts the app using the git tree from the previous successful deploy. It does not git-revert — it just checks out the prior commit and runs the deploy workflow from step 3 (build → start → health → gateway).

## Webhook receiver

The webhook receiver is a separate axum server on port 9021 (binds `0.0.0.0`). It has a single shared secret configured via `--webhook-secret` (or generated on first run and stored in the DB).

Each app can optionally have its own webhook secret (stored in `apps` table as `webhook_secret`). If not set, the global secret is used.

When a push webhook arrives:
1. Validate HMAC signature
2. Match the payload repo URL against registered apps
3. Spawn a `tokio::spawn` deploy task for each matched app (non-blocking — returns 202 immediately)
4. The deploy task emits WS events that the frontend picks up

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

By default, the API and UI listen on `127.0.0.1` only — no authentication needed. For remote access (`--bind 0.0.0.0`), a Bearer token is required:

```
Authorization: Bearer sk-postlab-...
```

The token is set via `--api-key` or generated on first run and printed to stdout. It's hashed in the DB, never logged.

The webhook receiver uses HMAC signature validation, not Bearer tokens.

### Running as root

The web server runs as root (same as the TUI). This is required because it spawns docker, systemctl, caddy, and other privileged commands. The frontend can only invoke operations through the API, which validates inputs and escapes shell arguments.

### Webhook secret

Stored in the `config` table (not in the `apps` table — it's a server-level setting). Generated on first `postlab go` run if not provided. Printed once; the user must configure it in their GitHub/GitLab webhook settings.

## Port conflict resolution

On startup, `postlab go` checks each port. If a port is in use:

1. Try the next available port (9020 → 9021 → 9022, etc.)
2. Print a warning: "Port 9020 in use, using 9021 instead"
3. If `--port` was explicitly set and is in use, error out immediately with a clear message

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

- [ ] New `cli/src/web/` module tree
- [ ] `postlab go` CLI subcommand in `main.rs`
- [ ] `AppState` with `SqlitePool` reference
- [ ] axum router with health check endpoint + WS placeholder
- [ ] `rust-embed` serving a placeholder `index.html`
- [ ] Port bind + graceful shutdown
- [ ] `make check` passes cleanly

### Phase 2 — Core API + DB

- [ ] `apps` / `app_env_vars` / `app_deploys` SQL migrations
- [ ] `core/apps/` module (CRUD, deploy, stop, start, rollback)
- [ ] `AppManager` that calls deploy/git/gateway
- [ ] REST API: apps CRUD + deploy + stop + start
- [ ] Env var endpoints
- [ ] `POST /api/v1/apps/:id/deploy` wired to git clone → detector → runner → gateway
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
- [ ] Webhook receiver (port 9021)
- [ ] HMAC validation + app matching
- [ ] Metrics endpoint (port 9022)

### Phase 6 — Runtime backends

- [ ] Detector expansion (Node, Python, Go, Rust, static)
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
