# `postlab go` — Tooling Dependency & Gap Analysis

## Scope & assumptions

- Target environment: **fresh Linux install** (Ubuntu/Debian, Fedora/RHEL, or Arch) with **none** of the runtime tools listed below pre-installed.
- `postlab` itself is installed as a single static binary via `install.sh` or equivalent.
- Focus: tooling gaps introduced by the `postlab go` web orchestration layer described in `docs/plan/postlab_go.md`, plus gaps in existing `cli/src/core/` adapters that become blocking once `postlab go` exercises them on a bare system.

## Executive summary

`postlab go` cannot function as a self-contained orchestrator on a fresh Linux install. It shells out to many external tools and relies on package managers that may not exist. Existing detection is spotty and installation is almost entirely manual. The concrete failure modes are:

1. **Startup failure**: `Platform::detect()` already bails if no supported package manager is found (`cli/src/core/platform.rs:126`).
2. **First deploy failure**: `git`, `docker`, `docker compose`, and `caddy` are required for the default docker-compose path; none are guaranteed.
3. **Backend-specific failure**: `pm2`, `k3s`, `kubectl`, `wash`, and language build toolchains are only needed for those runtimes, but the wizard currently offers no install path.
4. **Frontend build failure**: `make build-release` will require `npm`/`node` once the SvelteKit SPA exists; `make check`/`make build` must keep working without it.
5. **Install script gap**: `install.sh` only downloads the `postlab` binary; it bootstraps no runtime dependencies.

## Dependency inventory

| Tool | Used by | Required? | Current detection | Proposed resolution |
|---|---|---|---|---|
| **git** | `core/deploy/git.rs`; `core/projects` clone/pull; rollback | Yes for any repo | bare `Command::new("git")` fails silently | `packages.which("git")` pre-flight; auto-install via package manager; fail deploy with actionable message |
| **docker** | `core/docker/cli.rs`; docker-compose backend | Yes for docker backend | `DockerCliManager::detect()` picks `docker` or `podman` (`cli.rs:19`) | pre-flight check; auto-install Docker CE or Podman via package adapters |
| **docker compose** plugin | docker-compose backend | Yes for docker backend | same as docker | install `docker-compose-plugin`; Podman has compose built-in |
| **podman** | `DockerCliManager` fallback | Optional alternative | `which("podman")` | offer as default on RHEL/Fedora; install via package manager |
| **caddy** | `core/gateway/caddy.rs`; all domains/TLS | Yes if `app.domain` set | `caddy version` (`caddy.rs:89`) | auto-install via apt/dnf/brew; add health/reload check and official repo keys |
| **node / npm / npx** | pm2 backend; `core/projects` scaffolding | Yes for pm2/Node apps | `command -v npx` check (`projects/mod.rs:187`) | auto-install Node LTS via package manager or NodeSource |
| **pm2** | pm2 runtime backend | Yes for pm2 backend | none | `npm install -g pm2` after Node; or package manager |
| **systemd** | systemd backend; `core/services` | Yes for systemd backend | `is_systemd_available()` (`services/mod.rs:9`) | fail gracefully if not systemd |
| **k3s** | k3s runtime backend | Yes for k3s backend | none | pre-flight; install via official `curl \| sh`; require explicit confirmation |
| **kubectl** | k3s backend | Yes for k3s backend | none | package manager or static binary fallback |
| **wash** | wasmcloud backend (`core/wasm_cloud/cli.rs`) | Yes for wasmcloud | `find_wash()` (`cli.rs:14`) | improve brew/curl/cargo logic with package adapters and static binary fallback |
| **wasmcloud host** | wasmcloud backend | Yes for wasmcloud | none | `wash up` or systemd unit; pre-flight |
| **nats-server** | `core/nats/mod.rs` (wasmcloud backbone) | Indirectly | `is_installed()` (`nats/mod.rs:37`) | existing auto-download static binary; requires `curl` + `unzip` |
| **build toolchains** (gcc, make, python3, go, cargo) | systemd backend builds; detector | Runtime-specific | none | detector records need; wizard prompts to install per language |
| **curl** | `install.sh`; `wasm_cloud`; `tailscale`; `nats` download | Yes (many paths) | used directly | pre-flight critical; auto-install via package manager; `wget` fallback only where easy |
| **unzip** | `core/nats/mod.rs` auto-download | Yes for wasmcloud/NATS | none | auto-install via package manager |
| **ssh / ssh-keygen** | `core/ssh`; git over SSH | Optional | used directly | package manager install; warn if missing |
| **systemctl** | `core/services`; caddy reload | Existing + go | `is_systemd_available()` | required on Linux; fail fast if missing |
| **tar** | `install.sh` extraction | Yes for install | assumed present | auto-install via package manager if missing |

## Gap analysis by `postlab go` component

### Web server startup (`cli/src/web/`)
- **Gap**: no `web/` code exists yet. `axum`, `tower`, `tower-http`, `rust-embed`, `dashmap` are workspace dependencies (`Cargo.toml:32`) but are **not** declared in `cli/Cargo.toml`, so the module will not compile until `cli/Cargo.toml` is updated.
- **Tooling impact**: runtime requires only the compiled binary, but release build requires Node/npm for frontend assets.

### App manager / deploy workflow (`core/apps/`, `core/deploy/`)
- **Gap**: `core/deploy/git.rs` uses bare `git pull`, not `--ff-only` as required by the plan (`git.rs:20`).
- **Gap**: `core/deploy/detector.rs` recognizes only docker-compose and wasmcloud (`detector.rs:6`); the plan needs Node/Python/Go/Rust/static.
- **Gap**: `core/deploy/runner.rs` only handles `DockerCompose`/`WasmCloud`/`Unknown` (`runner.rs:8`).
- **Tooling impact**: every new runtime backend needs prerequisite detection + install hooks.

### Runtime backends

| Backend | Missing prerequisite handling |
|---|---|
| docker-compose | no check that `docker compose` plugin is installed separately from `docker` |
| pm2 | no detection or install of Node/npm/pm2 |
| systemd | no detection of build toolchains; no language-specific build step |
| k3s | no detection or install of k3s/kubectl |
| wasmcloud | `wash` install exists but host/NATS setup is fragile |
| static | only needs `caddy` if domain is set; otherwise no runtime |

### Gateway (`core/gateway/caddy.rs`)
- **Gap**: `CaddyManager::install()` exists but is only invoked from TUI screens, not from a headless `postlab go` deploy. It does not install the official Caddy repo GPG key on Debian, which can fail on fresh systems.
- **Gap**: no reload-health check after route mutation (`reload()` can fall back to `caddy start` but does not verify the route is reachable).

### Frontend build (`web/`)
- **Gap**: `web/` directory does not exist yet.
- **Gap**: `Makefile` `build-release` does not run `npm ci && npm run build` (`Makefile:35`).
- **Gap**: no placeholder `web/dist/index.html` committed, so `rust-embed` will break `make check` until the frontend is built unless `debug-embed` is configured.

### Install script (`install.sh`)
- **Gap**: only installs the `postlab` binary. No optional runtime bootstrap.

## Resolution strategies

### 1. Prerequisite detection layer

Introduce a new `core/tooling/` module (or `core/prereqs.rs`) with a uniform API:

```rust
pub struct ToolRequirement {
    pub name: &'static str,
    pub bin: &'static str,
    pub min_version: Option<&'static str>,
    pub install_cmd: InstallCmd,
    pub optional: bool,
}

pub async fn check(tool: &ToolRequirement) -> ToolStatus;
pub async fn ensure(tool: &ToolRequirement) -> Result<()>;
pub async fn health(tool: &ToolRequirement) -> HealthStatus;
```

Use existing `core::packages::which()` for PATH checks and add version parsing where meaningful (docker ≥ 24, caddy ≥ 2, etc.).

### 2. Automatic installation

Leverage the existing `PackageManager` trait (`AptManager`, `DnfManager`, `PacmanManager`, `BrewManager`) and extend it with a generic `install_many(&[&str])` helper. Map each runtime to OS-specific package names:

| Tool | Debian/Ubuntu | Fedora/RHEL | Arch | Notes |
|---|---|---|---|---|
| git | `git` | `git` | `git` | trivial |
| docker | `docker.io` or Docker CE repo | `docker-ce` (add repo) | `docker` | prefer distro package; root required |
| docker-compose-plugin | `docker-compose-plugin` | `docker-compose-plugin` | `docker-compose` | Debian needs Docker repo |
| podman | `podman`, `podman-compose` | `podman`, `podman-compose` | `podman`, `podman-compose` | preferred on RHEL |
| caddy | `caddy` (official repo) | `caddy` (COPR) | `caddy` (AUR) | improve key setup |
| node/npm | `nodejs`, `npm` or NodeSource | `nodejs`, `npm` | `nodejs`, `npm` | also set up `~/.npm-global` for global pm2 |
| pm2 | `npm install -g pm2` | same | same | after Node install |
| k3s | `curl -sfL https://get.k3s.io \| sh` | same | same | privileged install |
| kubectl | `kubectl` package | `kubectl` | `kubectl` | often bundled with k3s |
| wash | existing brew/curl/cargo logic | same | same | improve to prefer package manager |

For tools not in distro repos, add a `ScriptInstaller` fallback that streams progress through the same `mpsc` channel used by package installs.

### 3. Bundled / static binaries

- **NATS**: already auto-downloads a static `nats-server` binary to `~/.local/bin` (`nats/mod.rs:58`). Extend this pattern to:
  - `caddy` official static binary as fallback when package managers fail.
  - `wash` static binary from GitHub releases.
  - `kubectl` static binary from Kubernetes releases.
- **Postlab binary**: keep `install.sh` single-binary model; do not bundle external tools into the `postlab` binary (keeps size small and avoids licensing issues).

### 4. Graceful degradation / fallbacks

| Scenario | Behavior |
|---|---|
| No package manager found | `postlab go` exits at startup with a clear message; TUI already bails at `detect_package_manager()` |
| Docker missing | offer to install Docker CE or Podman; if declined, docker-compose backend is unavailable |
| Caddy missing | install automatically; if install fails, deployments with `app.domain` fail with a domain/TLS-specific error |
| Node/npm missing | auto-install on first PM2/Node scaffold; if declined, those backends unavailable |
| k3s refused | mark backend unavailable; user can retry from wizard |
| No systemd | disable systemd backend; show warning; macOS uses limited `MacosServiceManager` |
| No internet / install fails | surface actionable error; do not silently degrade a required backend |

### 5. Pre-flight health checks

Add `postlab go --doctor` and/or `GET /api/v1/health/tools` that returns JSON diagnostics for every tool. The web UI can then show a "Missing tools" banner with one-click install buttons. Example output:

```json
{
  "git": { "installed": true, "version": "2.43.0" },
  "docker": { "installed": false, "installable": true, "reason": "not in PATH" },
  "caddy": { "installed": false, "installable": true, "reason": "not in PATH" },
  "node": { "installed": true, "version": "20.12.0" },
  "pm2": { "installed": false, "installable": true, "reason": "npm available" }
}
```

## Recommended implementation phases

### Phase A — Prerequisite harness
1. Create `core/tooling/mod.rs` with `ToolRequirement`, `ToolStatus`, `check()`, `ensure()`, and `health()`.
2. Add `PackageManager::install_many()` to `AptManager`, `DnfManager`, `PacmanManager`, `BrewManager`.
3. Implement OS-specific package name maps for git, docker, docker-compose-plugin, podman, caddy, node/npm.
4. Add `postlab go --doctor` CLI and `GET /api/v1/health/tools` route.

### Phase B — Per-backend prerequisite enforcement
1. `RuntimeBackend::prerequisites(&self) -> Vec<ToolRequirement>`.
2. `AppManager::deploy()` calls `ensure_all()` before git/build/start.
3. Implement backend-specific install hints in the wizard/API error responses.

### Phase C — Frontend build safety
1. Create `web/` SvelteKit project.
2. Commit `web/dist/index.html` placeholder.
3. Add `rust-embed` `debug-embed` feature to `cli/Cargo.toml`.
4. Update `Makefile` so `build-release` runs `npm ci && npm run build` in `web/`.

### Phase D — Install script
1. Add optional `--bootstrap` flag to `install.sh` that installs git, docker, caddy, node, and pm2 via the detected package manager.
2. Keep default behavior minimal (single binary).

### Phase E — Health endpoint
1. `GET /api/v1/health/tools` returns JSON diagnostics.
2. UI shows a "Missing tools" banner with one-click install buttons.

## Open questions / risks

1. **Root vs. user installs**: PM2 and Node global installs are cleaner as a non-root user, but `postlab go` runs as root. Decide whether to install Node/PM2 system-wide or under the invoking user's home.
2. **Docker rootless**: Docker CE install requires root; rootless mode is out of scope for v1 but should be documented.
3. **k3s single-node**: k3s install is invasive (installs a systemd unit, opens ports). Should the wizard require explicit confirmation?
4. **Caddy official repo**: adding third-party APT/COPR repos in an automated installer is a security decision; document it and allow opt-out.
5. **Build toolchains**: installing `build-essential`, `golang`, `python3`, `cargo` can be large. Consider lazy install only when detector finds the matching language.
6. **WASM plugins future**: if third-party WASM plugins are added later, their tooling prerequisites must be added to the same harness.

## Verification checklist

Before `postlab go` ships:

- [ ] `make check` passes on a clean clone without Node/npm.
- [ ] `make build-release` builds the frontend and embeds it.
- [ ] `postlab go --doctor` reports expected tool statuses on a fresh VM.
- [ ] Deploying a docker-compose template on a fresh Ubuntu VM succeeds end-to-end after confirming installs.
- [ ] Deploying a PM2 app on a fresh VM succeeds after Node/PM2 auto-install.
- [ ] Missing `git`, `docker`, `caddy` each produce actionable errors in the API and UI.
