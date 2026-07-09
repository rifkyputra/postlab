# postlab go — Implementation Roadmap

Execution sequence for the design in [`postlab_go.md`](./postlab_go.md). That document is
the source of truth for *what* `postlab go` is; this one is the source of truth for *in what
order it gets built and how each step is proven done*. When the two disagree on behavior, the
design doc wins; when they disagree on sequencing, this doc wins.

Companion analysis: [`../research/postlab_go_edge_cases.md`](../research/postlab_go_edge_cases.md),
[`../research/postlab_go_alternatives.md`](../research/postlab_go_alternatives.md),
[`../research/postlab_go_diagrams.md`](../research/postlab_go_diagrams.md),
[`../research/postlab_go_tooling_problems.md`](../research/postlab_go_tooling_problems.md).
Detailed design for the first feature phase: [`postlab_git_rewrite.md`](./postlab_git_rewrite.md).

> **The git wrapper comes first.** Every deploy, rollback, and webhook in this plan begins with
> a git operation, and today that path is a dead `bare git pull` stub plus a pile of duplicated
> shell strings in `core/projects/mod.rs`. So **Phase 1 is the git rewrite** — a single typed,
> testable `GitRepo` — built before the web server, the schema, or anything else. It depends on
> nothing but the `git` CLI and unblocks the entire deploy pipeline.

> **WASM is the first-class deployment target.** The walking skeleton (Phase 5 / M1) deploys a
> **wasmcloud** app end-to-end. Every other runtime — docker-compose, systemd, pm2, k3s, static
> — is a later phase (Phase 8). This matches where the repo already invests: `wasm_cloud/cli.rs`
> (`find_wash` + install), a full NATS lifecycle in `nats/mod.rs`, and `runner.rs`'s existing
> `wash app deploy` path give wasmcloud more real scaffolding than any other backend today.

> **Tooling is a hard dependency, not a footnote.** `postlab go` shells out to git, `wash`, a
> running wasmcloud host, nats-server (and later docker, caddy, node/pm2, k3s/kubectl, language
> build toolchains) — **none guaranteed on a fresh Linux install**. The tooling-problems doc
> shows the deploy path fails on a bare box without its tools present. That is why a
> **prerequisite harness (Phase 4) lands before the walking skeleton (Phase 5)**, and why
> per-backend prereq enforcement is woven into the backends phase rather than deferred.

## Canonical on-disk layout

One layout, used by every phase, chosen to be FHS-correct for a root daemon and to keep
generated artifacts **outside** the git working tree (so a force-checkout/`git clean` on
rollback can never wipe configs or logs):

```
/var/lib/postlab/
├── .gitconfig                 # GIT_CONFIG_GLOBAL — postlab-controlled identity/helper
├── .ssh/known_hosts           # root-managed, not the invoking user's ~/.ssh
└── apps/<id>/
    ├── repo/                  # git working tree (clone target) — only git touches this
    ├── deploy_key             # ed25519 private key, mode 0600 (NOT inside repo/.git)
    ├── generated/             # wadm.yaml / compose / .env — safe from git clean
    └── deploys/<deploy_id>.log
```

> **Doc-sync note:** `postlab_go.md` currently says `config_dir = ~/postlab/apps/<id>/` with
> logs under it. That must be reconciled to the `/var/lib/postlab` layout above (the
> `apps.config_dir` column then stores `/var/lib/postlab/apps/<id>`). Tracked as a Phase 0 fix.

## How to read this

- The original six-phase plan is expanded into **Phases 0–14** with lettered sub-phases.
  A sub-phase is the smallest unit that lands on its own with `make check && make test` green.
- Each phase lists **Depends on**, **Work**, **Exit gate**, and **Risks**. A phase is not
  "done" until its exit gate is met — not when the code merely compiles.
- Phases are ordered so the system is **end-to-end demonstrable as early as possible**
  (walking skeleton at M1), not so that whole layers land at once.

### Shippable milestones

| Milestone | Reached after | What works |
|---|---|---|
| **M1 — Headless WASM deploy** | Phase 5 | `postlab go` serves an authed API, **detects and (with consent) installs `wash` + nats + a wasmcloud host on a bare box**, then deploys a **wasmcloud** app end-to-end (git → `wash app deploy` → readiness → Caddy) with no browser. |
| **M2 — Usable web UI** | Phase 7 | Dashboard, new-app wizard, env editor, deploy history, a **"missing tools" banner with one-click install**, and **live** streaming deploy/logs in the browser. |
| **M3 — Multi-runtime + push-to-deploy** | Phase 10 | docker-compose/systemd/static/pm2/k3s backends (each enforcing its own prerequisites), zero-downtime swaps, and GitHub/GitLab webhooks. |
| **GA — 1.0** | Phase 12 | Template catalog and Prometheus metrics/sparklines. |
| Post-1.0 | Phase 13+ | `install.sh --bootstrap`, Tailscale Funnel, WASM plugins, daemon mode. |

### Non-collapsible gates (from CLAUDE.md, apply throughout)

- **QAS** — every sub-phase ends with `make check` (zero warnings) **and** `make test` green.
- **Schema confirmation** — the `apps`/`app_env_vars`/`app_deploys` tables ship as inline
  `CREATE TABLE IF NOT EXISTS` in `db/mod.rs` (Phase 3), **not** files under `migrations/`.
  No `.sql` file is created without user confirmation.
- **Security review** — every system-mutating diff (git, docker, systemd, Caddy, webhook
  receiver, auth) gets a cold-agent security review before merge.
- **Manual web verification** — the web UI can't be verified headlessly, the same way TUI
  screens can't. Any phase touching `web/` ends with an explicit "needs `sudo postlab go` +
  browser check" note; do not claim a UI works without it.
- **CI / `install.sh` confirmation** — `make build-release` frontend integration (Phase 6f)
  and any CD matrix change require user confirmation before touching `.github/workflows/` or
  `install.sh`.
- **`feature_list.json`** — updated whenever a screen, tab, or CLI command is added/renamed
  (Phase 1 `postlab git …` commands, Phase 2 `go` command, Phase 7 web pages).

---

## Phase 0 — Dependency & repo prep

The cheapest way to de-risk the build: get every dependency and build-gate concern out of the
way first, in one small, reviewable diff that changes no behavior. Independent of Phase 1 —
either can land first.

**Depends on:** nothing.

**Work** — all done (2026-06-30)
- [x] Workspace `Cargo.toml`: `axum`, `tower`, `tower-http`, `thiserror`, `rust-embed`
      were already pinned; added `dashmap = "6"`, `arraydeque = "0.5"`, `uuid v7` feature,
      `debug-embed` feature to `rust-embed`.
- [x] `cli/Cargo.toml`: consuming `axum`, `tower`, `tower-http`, `rust-embed`, `dashmap`
      from workspace (wired in Phase 2; pinned here so workspace resolves consistently).
- [x] `web/dist/index.html` placeholder committed.
- [x] **Doc-sync `postlab_go.md`**: `app.deploy_type` → `app.runtime`;
      `~/postlab/` paths → `/var/lib/postlab/`; SvelteKit + adapter-static already correct.

This phase also satisfies the **frontend-build-safety** items from the tooling doc (its
"Phase C"): the committed `web/dist/` placeholder + `debug-embed` are exactly what keep
`make check`/`make build` working on a clone with no node installed.

**Exit gate**
- `make check && make test` green with the new deps present but unused (allow `dead_code`
  where needed, removed as each is consumed).
- Fresh `git clone` + `make build` succeeds **without** running any npm command.

**Risks**
- `debug-embed` + `compression` feature interaction — verify a release build still embeds and
  compresses real assets (re-checked at Phase 6f).

---

## Phase 1 — Git wrapper rewrite (foundational, **before everything**)

Full design: [`postlab_git_rewrite.md`](./postlab_git_rewrite.md). Replace the dead
`core/deploy/git.rs` stub and the duplicated shell logic in `core/projects/mod.rs` with one
typed `core/git/` wrapper (`GitRepo`) handling clone, `--ff-only` pull, checkout-by-SHA,
dirty-tree detection, and authenticated remotes. This is first because **every** deploy,
rollback, and webhook starts with a git op.

**Depends on:** the `git` CLI only — no web server, no DB schema, no AppState. Runs in parallel
with Phase 0.

### 1a — Core wrapper
- [ ] New `core/git/` (`repo.rs`, `creds.rs`, `error.rs`). `GitRepo::{new, clone, pull_ff_only,
      checkout, current_sha, is_dirty, remote_sha}`, progress via `mpsc::UnboundedSender<String>`.
- [ ] **`run_as` dimension on `GitRepo` from day one** — `Root` (deploy, repos under
      `/var/lib/postlab`) vs `User(uid)` (project browser, repos user-owned). This is *the*
      decision that can't be retrofitted later; it splits ownership, `known_hosts`, and
      `GIT_CONFIG_GLOBAL`. See [open decision](#open-decisions).
- [ ] `clone` takes **branch + optional shallow depth** (the `apps.repo_branch` column needs it;
      `--depth 1` for deploy speed).
- [ ] Typed `GitError` (`thiserror`) on the public ops — `Result<_, GitError>`, not bare
      `anyhow` — so the deploy WS shows "Working tree has local changes", not "status 1".
- [ ] Env hardening on every invocation: `GIT_TERMINAL_PROMPT=0`, `GIT_SSH_COMMAND`,
      `GIT_CONFIG_GLOBAL=/var/lib/postlab/.gitconfig`.
- [ ] **Delete `core/deploy/git.rs` outright** — it has zero external callers, so no re-export
      bridge is needed (keeping one would unfulfill its `#[expect(dead_code)]` and fail `make check`).

### 1b — Auth & keys
- [ ] `GitCreds::{None, HttpsToken, SshKey}`. **Tokens go to a mode-0600 credential file /
      helper, never embedded as `https://<token>@host/...`** (that leaks to `ps`, reflog, and the
      persisted `.git/config`). Keys at `/var/lib/postlab/apps/<id>/deploy_key` (mode 0600,
      **alongside** the repo, not inside `repo/.git`).
- [ ] `postlab git deploy-key --app <id>` / `allow-host <host>` (ssh-keyscan → managed
      `known_hosts`) / `set-token --app <id>` (token via stdin/file, not argv).
- [ ] Update `feature_list.json` with the three `postlab git …` subcommands.

### 1c — Refactor existing callers
- [ ] Rewrite `core/projects/mod.rs::{clone_repo, pull_project}` (live code, called from
      `tui/app.rs:3881`) to delegate to `GitRepo` with `run_as: User` — **preserving today's
      user-owned checkout behavior** (it currently runs git via `su`). Delete the duplicated
      shell-quoting / `GIT_SSH` strings.
- [ ] Keep `git_status`/`set_git_identity`/`set_github_token` as thin shims over `creds.rs`.
      Rename the wrapper's git-binary status type to **`GitInstall`** to avoid colliding with the
      existing repo-status `GitStatus`.

**Exit gate**
- Unit tests against a bare repo in `tempfile::tempdir()`: clone (with branch); `pull_ff_only`
  succeeds on fast-forward and **fails `GitError::DirtyTree`** on a dirty tree; `checkout` to a
  prior SHA and back.
- TUI project browser still clones/pulls as the invoking user (no ownership regression).
- `core/deploy/git.rs` gone; `core/projects/mod.rs` no longer shells out to `git` directly.
- `make check && make test` green. Security review (creds/key handling).

**Risks**
- Concurrency: `pull`/`checkout` mutate the tree and must run under the per-app deploy lock
  (Phase 3c) — the wrapper is **not** independently concurrency-safe.
- `checkout(force)` cleaning untracked files is safe **only** because generated artifacts live in
  `generated/`, outside `repo/` (canonical layout). Do not relax that.

---

## Phase 2 — Server scaffold + auth

Stand up the `postlab go` process: it binds, authenticates, serves the embedded SPA shell,
and exits cleanly. No app logic yet.

**Depends on:** Phase 0.

### 2a — CLI + lifecycle
- [ ] `postlab go` subcommand in `main.rs` with `--port` / `--metrics-port` / `--bind` /
      `--webhook` / `--webhook-port` / `--api-key-file`. (Root check stays — do not weaken.)
- [ ] `AppState { db, platform, app_manager, ws_registry }` skeleton (managers stubbed).
- [ ] Port-bind with **reserved-port-skipping** fallback (API 9020→9023+, webhook 9021→9031+,
      metrics 9022→9041+; never stomp the other two defaults). Explicit `--port` in use →
      hard error, never silent move.
- [ ] Graceful shutdown on SIGINT/SIGTERM (close WS, drain ≤5s, flush WAL, exit 0).
- [ ] Update `feature_list.json` with the `go` command.

### 2b — Auth middleware (day one, not deferred)
- [ ] Bearer token: generated on first run, printed **once** to stdout, stored **hashed** in
      DB. Hash scheme is deterministic — `SHA-256(token)` (edge-case 1.1) — *not* a password
      hash, since the raw token is discarded.
- [ ] Token source precedence: `POSTLAB_API_KEY` env / `--api-key-file` > persisted hash.
      Trim trailing whitespace/newlines from the file (edge-case 2).
- [ ] Origin/Host allow-list middleware (loopback + configured bind). Decide `Origin: null`
      handling explicitly (reject by default).
- [ ] `postlab go --rotate-token` (or equivalent) so a lost token doesn't require DB surgery.

### 2c — Asset serving + transport skeleton
- [ ] `rust-embed` `WebAssets` from `web/dist/`; serve embedded bytes via axum.
- [ ] SPA fallback: all `/ui/*` → `index.html`; API under `/api/v1`.
- [ ] `GET /healthz` (unauthed liveness) + authed `GET /api/v1/ping`.
- [ ] WS endpoint placeholder at `/ws` (upgrade handshake only; no events yet).

**Exit gate**
- `curl http://127.0.0.1:9020/api/v1/ping` → 401 without token, 200 with token.
- Cross-origin / bad-Host request → 403.
- Ctrl+C shuts down within the drain window, exit 0.
- `make check && make test` green.

**Risks**
- Auth middleware ordering (Origin check must run *before* token lookup to avoid a DB hit on
  rebinding attempts).

---

## Phase 3 — Data model + app CRUD + concurrency primitives

**Depends on:** Phase 2.

### 3a — Schema
- [ ] `apps`, `app_env_vars`, `app_deploys` as inline `CREATE TABLE IF NOT EXISTS` in
      `db/mod.rs` (or a `db/apps.rs` helper it calls). **Confirm with user** before touching
      `migrations/` — these do **not** go there.
- [ ] Add a comment in `db/mod.rs` noting `migrations/` are not applied at runtime and inline
      schema must be kept in sync (alternatives doc §2 recommendation).
- [ ] Validate `app.id` / `app_id` as `^[a-z0-9_-]+$` at creation (edge-case 1.10 — load-bearing
      for systemd unit names later).

### 3b — CRUD module + REST
- [ ] `core/apps/` module: create / get / list / delete (cascade) — pure DB, no deploy yet.
- [ ] REST: `GET|POST /api/v1/apps`, `GET|DELETE /api/v1/apps/:id`.
- [ ] Env endpoints: `GET|PUT /api/v1/apps/:id/env`, `DELETE …/env/:key` (mask `secret=1`).
- [ ] Per-app `webhook_secret` generated at create time; surfaced once on read.

### 3c — Concurrency primitives
- [ ] `AppManager` holds `DashMap<app_id, Mutex<()>>` deploy locks (edge-case 1.5); acquire
      before any deploy/rollback/start/stop. The Phase 1 `GitRepo` ops run **inside** this lock.

**Exit gate**
- Integration test: create → list → get → put env → delete (cascade verified) over HTTP.
- `make check && make test` green.

**Risks**
- Cascade delete must also remove the on-disk `/var/lib/postlab/apps/<id>/` tree (repo, keys,
  generated configs, deploy logs), not just rows.

---

## Phase 4 — Prerequisite harness (tooling bootstrap)

The walking skeleton can't run on a fresh box until the system can **detect and install the
tools it shells out to**. This phase builds that harness once, so every later phase declares
its tools instead of failing at a bare `Command::new(...)`. Source: tooling-problems doc
§"Resolution strategies" 1/2/5 (its Phases A + E).

**Depends on:** Phase 3.

### 4a — Detection + ensure layer
- [ ] New `core/tooling/` module: `ToolRequirement { name, bin, min_version, install_cmd,
      optional }`, plus `check()` (PATH + version via existing `packages::which()`), `ensure()`
      (install if missing), `health()` (post-install reachability).
- [ ] **Register `git` as a `ToolRequirement`** (tooling doc §"git", git-rewrite Phase 5) — the
      Phase 1 wrapper reports its `GitInstall` status through here.
- [ ] Version parsing where it matters (docker ≥ 24, caddy ≥ 2); skip where it doesn't.

### 4b — Installation backend
- [ ] Add `PackageManager::install_many(&[&str])` to `AptManager`/`DnfManager`/`PacmanManager`/
      `BrewManager` (today only `install`/`install_streamed` exist). Reuse the existing
      `install_streamed` mpsc progress channel.
- [ ] **M1-critical toolset first**: git, `wash` (reuse `wasm_cloud/cli.rs` install logic),
      nats-server (reuse `nats/mod.rs` auto-download), and the wasmcloud host. The
      docker/compose/podman/caddy/node maps land here too but only gate later backends.
- [ ] `ScriptInstaller` + static-binary fallback (extend the existing `nats/mod.rs` curl-download
      pattern) for tools not in distro repos — `wash`, caddy static binary, kubectl.
- [ ] **Caddy repo/GPG-key setup** on Debian (the existing `CaddyManager::install` skips it and
      fails on fresh systems) — and document the third-party-repo trust decision with an opt-out.

### 4c — Doctor + diagnostics endpoint
- [ ] `postlab go --doctor` CLI: prints per-tool installed/version/installable status (git included).
- [ ] `GET /api/v1/health/tools` returning the same as JSON (consumed by the UI banner in P7).
- [ ] Graceful-degradation matrix (tooling doc §4): no package manager → exit with clear message
      (TUI already bails this way); missing required tool for a chosen backend → actionable error,
      never silent degradation.

**Exit gate**
- On a fresh VM with no `wash`/nats/host, `postlab go --doctor` reports them missing +
  installable; `ensure()` installs them; `health()` confirms a reachable wasmcloud host
  (`wash` can talk to it) afterward. The docker/caddy paths are exercised by a unit test even
  though they don't gate M1.
- `make check && make test` green. Security review (this code adds third-party repos and runs
  privileged installs, including the wasmCloud `curl | bash` script path).

**Risks**
- **`curl | bash` install for `wash`** (existing `wasm_cloud/cli.rs` path) — pin/verify where
  possible; document the trust decision alongside the Caddy-repo one.
- **Root vs. user installs** for node/pm2 (open question 1) — decide system-wide vs. invoking
  user's home now, since 4b's node mapping sets the precedent (and ties to Phase 1's `run_as`).
- Lazy install of large build toolchains (Rust/TinyGo for `wash build`, plus gcc/go/cargo/python
  for later backends) is deferred to the phases that need them, not done here.

---

## Phase 5 — Walking skeleton: **wasmcloud** deploy pipeline (**M1**)

The keystone phase. One backend, end-to-end, no browser. **wasmcloud is first-class** and the
backend the walking skeleton proves: it has the most existing scaffolding in the repo
(`wasm_cloud/cli.rs`, `nats/mod.rs`, `runner.rs`'s `wash app deploy`), and the simplest path
(a `wadm.yaml` referencing pre-built OCI components) sidesteps any local build, the same way a
compose image would. The conventional runtimes are deliberately deferred to Phase 8.

**Depends on:** Phase 1 (`GitRepo`), Phase 3 (apps schema + deploy lock), Phase 4 (the deploy
path calls `ensure()` for git + `wash` + nats + a running wasmcloud host — and caddy if
`app.domain` is set — before doing anything; this is what makes M1 work on a bare box).

### 5a — Wire `GitRepo` into deploy
- [ ] Use the Phase 1 `GitRepo` (`run_as: Root`, clone target `/var/lib/postlab/apps/<id>/repo`):
      clone if `repo/.git` absent, else `pull_ff_only`. Resolve and record the SHA. No new git
      code here — this is purely consumption of Phase 1.

### 5b — RuntimeBackend trait + dispatch
- [ ] Define the trait per design doc, including **`supports_http_health()`** opt-out (wasmcloud
      readiness is a wadm/host status, not necessarily an HTTP GET — see 5e).
- [ ] `AppManager` dispatch on `app.runtime` (single backend registered for now).

### 5c — wasmcloud backend
- [ ] Ensure a wasmcloud host + NATS are running (reuse `nats/mod.rs` `start_async` /
      `init_wasmcloud_buckets_async`; `wash up` for the host). Treat host lifecycle as part of
      the backend, not the per-app deploy.
- [ ] Generate/validate `wadm.yaml` **into `generated/`** (outside the git tree) if the repo
      lacks one; deploy via `wash app deploy` (the existing `runner.rs` path). The simple M1 path
      uses a wadm manifest pointing at published OCI components — `wash build` from source is a
      later concern (toolchain-gated, like the systemd build story in 8c).
- [ ] **Fill the teardown stub**: `runner.rs` currently bails on WasmCloud stop ("needs app name
      parsing") — parse the app name from `wadm.yaml` and `wash app delete` / `undeploy`.
- [ ] status/logs via `wash app get` / host logs.

### 5d — Deploy workflow engine
- [ ] **Prereq gate (first step):** call Phase 4 `ensure()` for git + `wash` + nats + host
      (+ caddy if `app.domain`) before touching the repo; abort with the doctor-style error if a
      required tool is missing and can't be installed.
- [ ] `AppManager::deploy` (under the Phase 3c lock): ensure → git → detect (wasmcloud only for
      now) → generate `wadm.yaml` → `wash app deploy` → readiness → gateway → complete. Writes
      `app_deploys` rows and a log file at `/var/lib/postlab/apps/<id>/deploys/<deploy_id>.log`.
- [ ] `POST /api/v1/apps/:id/deploy`, `…/stop`, `…/start`.

### 5e — Readiness / health checker
- [ ] wasmcloud readiness comes from **wadm/host status** (`wash app get` → scaled & healthy),
      not a blind HTTP poll, since not every component exposes an HTTP server. If the app uses
      the httpserver capability on a port, additionally poll `GET :port{health_path}` every
      500ms with a **configurable** timeout (default 30s, edge-case 3). On failure: mark deploy
      failed, leave the prior app version deployed.

### 5f — Gateway integration
- [ ] When `app.domain` is set **and** the component exposes an httpserver port, call existing
      `CaddyManager::add_route` → `localhost:{port}`; reload. (CaddyManager is real today — wire,
      don't build.) Caddy presence is guaranteed by the 5d prereq gate. Components with no HTTP
      surface skip the gateway step.

**Exit gate (M1)**
- On a fresh VM, from a clean DB: `POST /apps` then `POST /apps/:id/deploy` against a real
  wasmcloud repo (wadm manifest referencing OCI components) brings the app to `running`
  (wadm reports it scaled/healthy), an httpserver component is reachable through Caddy, and a
  deploy log is on disk. `…/stop` tears it down cleanly (no more "not implemented" bail).
  Demonstrated headless (curl only).
- `make check && make test` green. Security review of the deploy/wash/gateway diff.

**Risks**
- Build output streaming isn't here yet (lands P7); deploy is synchronous/log-file-only for now.
- Caddy auto-TLS needs a real domain — document a localhost/`:port` smoke path for CI.
- wasmcloud host lifecycle: deciding host ownership (postlab-managed vs. pre-existing) and
  cleanup on `postlab go` shutdown. Per the design doc, running apps keep running — so the host
  is **not** torn down on shutdown.

---

## Phase 6 — Frontend MVP (**toward M2**)

**Depends on:** Phase 5 (a real API to talk to), Phase 4 (`/api/v1/health/tools` for the banner).

### 6a — Scaffold
- [ ] `web/` SvelteKit project, `adapter-static` SPA (`fallback: index.html`, prerender off).
- [ ] `lib/api.ts` (typed fetch wrappers), `lib/ws.ts` (client w/ reconnect, stubbed until P7),
      `lib/stores.ts`.

### 6b — Login + token handling
- [ ] Login screen: token entered, held **in memory only** (document re-entry on reload).

### 6c–6e — Pages
- [ ] Dashboard: app list + system overview gauges (from Platform).
- [ ] **"Missing tools" banner** reading `GET /api/v1/health/tools`, with one-click install
      buttons (tooling doc "Phase E" UI half).
- [ ] New-app wizard (runtime → source → domain → env → review). Surface per-backend prereq
      status here once Phase 8 lands `prerequisites()`.
- [ ] App detail overview (status, domain, latest deploy), env editor (masking), deploy history.
- [ ] Update `feature_list.json` with each web page.

### 6f — Build integration
- [ ] `make build-release` runs `npm ci && npm run build` → `web/dist/`, then cargo. `make
      build`/`make check` stay npm-free (debug-embed). **Confirm before editing CI** if the CD
      matrix needs the node toolchain.

**Exit gate**
- `sudo postlab go` + browser: create an app via the wizard and watch it deploy (poll-based
  until P7). **Manual verification required — cannot be headless.**
- `make check && make test` green.

**Risks**
- SPA base-path / API-URL handling under the embedded server (alternatives §4 caveat).
- Release embed path (6f) is the first time real assets compile in — re-verify Phase 0's
  debug-embed/compression interaction.

---

## Phase 7 — Live streaming (**M2**)

**Depends on:** Phase 6.

- [ ] `WsRegistry` (`DashMap<app_id, Vec<Sender>>`); real `/ws` event broadcast.
- [ ] Deploy workflow emits `deploy.progress` line-by-line. **Split on `\r` as well as `\n`**
      so `npm ci`-style carriage-return progress streams correctly (edge-case 3).
- [ ] Streaming deploy log in wizard + detail page.
- [ ] `app.status` events; live per-app log viewer (`wash app get` / host logs; later
      `docker compose logs -f` per backend).

**Exit gate (M2)**
- Browser shows a deploy streaming in real time and live app logs. Manual verification.
- `make check && make test` green.

**Risks**
- Backpressure / slow consumers on the WS registry; cap buffered lines per client.

---

## Phase 8 — Conventional runtime backends

Everything that isn't wasmcloud. Each sub-phase is independently shippable and ends by deploying
a representative repo. Ordered easiest-first. wasmcloud already shipped in Phase 5.

**Depends on:** Phase 5 (trait + dispatch, wasmcloud reference impl), Phase 4 (prereq harness).

- [ ] **8.0 — per-backend prereqs**: add `RuntimeBackend::prerequisites(&self) ->
      Vec<ToolRequirement>` and have `AppManager::deploy` call `ensure_all()` for the selected
      backend before git/build/start (tooling doc "Phase B"). wasmcloud's prereqs (defined in
      Phase 5) move under this interface too. Each backend below declares its own.
- [ ] **8a — static**: Caddy config → repo dir, no process, `supports_http_health() = false`;
      UI must not show a failing health badge (edge-case 4). *Prereqs:* caddy (if domain).
- [ ] **8b — docker-compose**: generate `docker-compose.yml` (+ minimal `Dockerfile` when the
      repo lacks one) with port mapping and env file; start/stop/status/logs via `docker compose …`.
      *Prereqs:* docker + `docker-compose-plugin` (or podman). The image carries the build, so no
      language toolchain needed.
- [ ] **8c — systemd**: generate `.service`, install to `/etc/systemd/system/<app_id>.service`,
      `systemctl` start/stop, `journalctl` logs. **Resolve the build story** for go/rust
      (where/whether the binary compiles, toolchain-present check) — the most underspecified
      part of the design. Relies on Phase 3a `app_id` validation. *Prereqs:* systemd present +
      **lazy** language toolchain install (gcc/make/go/cargo/python) only when the detector
      finds that language (tooling doc open-question 5).
- [ ] **8d — detector rewrite**: wasmcloud (already) + Node / Python / Go / Rust / compose /
      static (replaces the 2-case stub). **wasmcloud markers (`wadm.yaml`/`wasmcloud.toml`) take
      precedence** so the first-class path wins; define precedence for the rest when multiple
      markers match (edge-case 3).
- [ ] **8e — pm2**: `ecosystem.config.js`; *prereqs:* node/npm then `npm i -g pm2`, installed
      only with **explicit consent**, honoring the root-vs-user decision from Phase 4 (root-install
      caveat, edge-case 4).
- [ ] **8f — k3s**: generate deployment/service/ingress, `kubectl apply`; *prereqs:* k3s +
      kubectl via the harness's script/static-binary fallback, **explicit confirmation** for the
      invasive k3s install (opens ports, installs a unit — open-question 3); **fail gracefully**
      if a cluster is absent (edge-case 4).

**Exit gate (per sub-phase)**
- On a fresh VM, the backend's `prerequisites()` install cleanly (or fail with an actionable
  message), then a representative repo deploys, runs, logs, and tears down.
- Security review for docker/systemd/pm2/k3s diffs (they write privileged units/manifests and add repos).
- `make check && make test` green.

---

## Phase 9 — Zero-downtime deploys

**Depends on:** Phase 8 (docker-compose from 8b; k3s from 8f).

- [ ] Temp-port + Caddy route-swap for **docker-compose and k3s only** (systemd/pm2 = restart,
      static = git pull, per alternatives §8). wasmcloud gets rolling updates natively via wadm
      scaling, so it's outside this temp-port mechanism.
- [ ] **Probe temp-port bind availability** before selecting it (edge-case 1.8).
- [ ] **Orphan cleanup**: if health passes but gateway update fails, stop the new
      container/pod and keep the old route (edge-case 1.9).
- [ ] Rollback flow: `GitRepo::checkout(previous_sha, force)` then re-run from build step (not
      git-revert). Safe because generated artifacts are outside `repo/` (canonical layout).

**Exit gate**
- A redeploy swaps with no dropped requests against a continuously-curled endpoint; an induced
  gateway failure leaves no orphan container and the old version still serving.
- `make check && make test` green. Security review.

---

## Phase 10 — Webhooks / push-to-deploy (**M3**)

Intentionally **after** backends and zero-downtime: a webhook is only meaningful once a real
deploy across runtimes works, and it exposes a root-privileged trigger to the internet, so it
gets the most scrutiny.

**Depends on:** Phase 8, Phase 9 (rollback).

- [ ] **10a** — opt-in receiver, separate axum server on `0.0.0.0:9021`, off unless `--webhook`.
- [ ] **10b** — GitHub handler: candidate set by repo URL, then validate HMAC against **every**
      candidate's own secret (repo URL never authorizes — edge-case 1.3). Per-app secret only,
      **no shared fallback**.
- [ ] **10c** — GitLab handler (token/secret header equivalent).
- [ ] **10d** — **persisted** replay/dedup: store delivery IDs in SQLite with a TTL (~24h),
      reject duplicates **across restarts**, and reject out-of-skew timestamps (edge-case 1.2).
- [ ] **10e** — `POST /api/v1/apps/:id/rollback` surfaced in API/UI; webhook passes the pushed
      SHA to `GitRepo::checkout` when it differs from `HEAD`.
- [ ] Each receiver request → 202 immediately, `tokio::spawn` deploy, WS events to UI.

**Exit gate (M3)**
- A real GitHub push triggers a deploy; a replayed delivery (same ID) is rejected after a
  server restart; a forged payload with a real repo URL but wrong HMAC is rejected 401.
- Security review (mandatory — internet-facing root trigger).
- `make check && make test` green.

---

## Phase 11 — Template catalog

**Depends on:** Phase 6 (UI). Per-template runtime gates the entry: wasmcloud templates need
only Phase 5; docker-compose templates (n8n etc.) need Phase 8b.

- [ ] `cli/src/web/templates.json` embedded at compile time.
- [ ] `GET /api/v1/templates`, `POST /api/v1/templates/:id/deploy` (`{domain}` substitution).
- [ ] Templates page; deploy flow = wizard with locked, pre-filled fields.
- [ ] Lead the catalog with **wasmcloud-native templates** (on-brand, deployable from Phase 5);
      then docker-compose apps: n8n, Uptime Kuma, NocoDB, Ghost, MinIO, Plausible, Nextcloud,
      WordPress.

**Exit gate**
- Deploy a wasmcloud template supplying only name + domain; reaches `running`. Manual UI check.
  (A docker-compose template — e.g. n8n — additionally verified once 8b lands.)
- `make check && make test` green.

---

## Phase 12 — Metrics & observability (**GA / 1.0**)

**Depends on:** Phase 8.

- [ ] Per-app in-memory ring buffer (`DashMap<app_id, ArrayDeque<MetricSample, 720>>`); 5s
      sampler task. **Not persisted** to SQLite (write-amplification / 1s-PK collision —
      alternatives §6).
- [ ] `GET /metrics` Prometheus text on `127.0.0.1:9022` (no auth → keep loopback-only; document
      firewall note, edge-case 2). App-level + system-level series.
- [ ] UI sparklines (`MetricsSpark.svelte`) reading the buffer; document that restart resets them.

**Exit gate (GA)**
- `curl 127.0.0.1:9022/metrics` returns valid Prometheus text; UI sparklines update live.
- `make check && make test` green.

---

## Phase 13 — `install.sh --bootstrap` (optional, gated)

Convenience, off the critical path — the Phase 4 harness already installs tools on demand at
deploy time; this just front-loads them at install time. Source: tooling doc "Phase D".

**Depends on:** Phase 4 (reuses the same package maps). Can land any time after.

- [ ] Optional `--bootstrap` flag on `install.sh` that installs git, wash, docker, caddy, node, pm2
      via the detected package manager. **Default behavior stays single-binary.**
- [ ] **`install.sh` is a CLAUDE.md-gated file — user confirmation required before editing.**
- [ ] Document the third-party-repo trust decision (Caddy APT/COPR, Docker CE, wash) with opt-out.

---

## Phase 14 — Post-1.0 / future (not scheduled)

Tracked, not committed. Each is its own design effort.

- `postlab go --funnel` — Tailscale Funnel for secure remote access.
- WASM plugins — third-party pages calling the REST API (their tooling prerequisites go through
  the same Phase 4 harness — tooling doc open-question 6).
- `postlab go --daemon` — daemonize (systemd/PID/journald) so TUI and web can run together;
  if pursued, keep the `AppState` shape so the inline→daemon extraction is mechanical
  (alternatives §1 recommendation).
- Stricter SSH host-key policy (move off `accept-new` TOFU); LFS / submodule support behind
  explicit flags (git-rewrite risks 2–3).

---

## Open decisions

All three decisions below were resolved during the Phase 1 architecture phase
(see [`postlab_git_rewrite.md` — Resolved decisions](./postlab_git_rewrite.md#resolved-decisions)):

1. **`GitRepo` ownership model (`run_as`) — resolved.** Both ownership modes supported via
   `RunAs::Root` (deploy repos under `/var/lib/postlab`) and `RunAs::User(uid)` (TUI project
   browser). Baked into `GitRepo` from day one; no global collapse to root.
2. **Canonical path — resolved to `/var/lib/postlab`.** `postlab_go.md` doc-sync done
   (2026-06-30): `~/postlab/` references replaced with `/var/lib/postlab/`.
3. **HTTPS token storage — resolved: per-app mode-0600 credential file, never URL-embedded.**
   Matches existing `core/projects/mod.rs` pattern. Token never appears in `ps`, reflog, or
   `.git/config`.

---

## Dependency graph

```mermaid
flowchart LR
    P0[P0 deps] --> P2[P2 scaffold+auth]
    P1[P1 git wrapper] --> P5
    P1 --> P4
    P2 --> P3[P3 model+CRUD]
    P3 --> P4[P4 prereq harness]
    P4 --> P5[P5 wasmcloud skeleton • M1]
    P5 --> P6[P6 frontend]
    P6 --> P7[P7 live streaming • M2]
    P5 --> P8[P8 other backends]
    P8 --> P9[P9 zero-downtime]
    P8 --> P10[P10 webhooks • M3]
    P9 --> P10
    P8 --> P11[P11 templates]
    P6 --> P11
    P8 --> P12[P12 metrics • GA]
    P4 --> P13[P13 install.sh bootstrap • optional]
    P7 -. UI surfaces .-> P8
    P7 -. UI surfaces .-> P11
```

**Phase 1 (git) is foundational and unblocks the deploy pipeline** — it feeds both the prereq
harness (git is a tracked tool) and the walking skeleton (deploy clones via `GitRepo`), and it
depends on nothing but the `git` CLI, so it can start immediately alongside Phase 0. The
**prereq harness (P4) gates the walking skeleton (P5)**. After P5, the frontend (P6) and
backends (P8) proceed in parallel, converging only where the UI surfaces a backend's runtime
(dotted edges). P13 (install.sh bootstrap) hangs off the harness and can land any time. The
P6/P8 split is the main opportunity to fan out work.
