# Postlab Architecture

## Overview

Postlab is a single-binary, bare-metal server manager with a terminal UI. It runs directly on the machine it manages, requires root, and is built with Rust using ratatui + crossterm and SQLite via sqlx.

```
cli/src/
├── main.rs                  # clap CLI parsing, root check, DB init
├── core/                    # Platform-agnostic domain logic (no TUI dependency)
│   ├── platform.rs          # Platform struct + detect() factory
│   ├── models.rs            # Shared data types (serde)
│   ├── system/              # SystemInfo trait + sysinfo impl
│   ├── packages/            # PackageManager trait + apt/dnf/pacman/brew
│   ├── processes/           # ProcessManager trait + sysinfo impl
│   ├── security/            # SecurityAuditor + Fail2Ban
│   ├── firewall/            # FirewallManager trait + ufw/firewalld/pf
│   ├── portcheck/           # External port reachability checker
│   ├── ssh/                 # SshKeyManager trait
│   ├── services/            # ServiceManager trait + systemd/launchd
│   ├── users/               # Unix user management
│   ├── docker/              # DockerManager trait + docker/podman CLI
│   ├── workloads/           # Host-managed single-service workloads
│   ├── wasm_cloud/          # wasmCloud CLI management
│   ├── nats/                # NATS backbone health checks
│   ├── ghost/               # Ghost service detection
│   ├── gateway/             # GatewayManager trait + Caddy impl
│   ├── tunnel/              # TunnelManager trait + cloudflared impl
│   ├── deploy/              # Git-based deployment (detector, git, runner)
│   └── pi_agent/            # Pi Agent CLI integration
├── db/
│   ├── mod.rs               # init_db (SQLite, auto-create)
│   ├── audit.rs             # Audit log persistence
│   └── deployments.rs       # Deployment CRUD
└── tui/
    ├── mod.rs               # Terminal init, render loop, nav/status bar
    ├── app.rs               # App state machine + background task dispatch
    ├── events.rs            # Keyboard dispatch + mouse click handler
    └── screens/             # One file per top-level screen
```

## Layer Separation

Postlab enforces a strict separation between `core/` (domain logic) and `tui/` (presentation). The `core/` layer has zero dependency on ratatui or crossterm and can be used independently (e.g., by a future HTTP API). The `tui/` layer owns all rendering, input handling, and application state.

### `core/` — Platform abstraction

`core/platform.rs` contains the `Platform` struct and a `detect()` factory function. At startup, `detect()` probes the host for available package managers, firewalls, init systems, and container engines, then instantiates the appropriate trait implementations. The result is an owned `Platform` that the TUI wraps in `Arc` for cheap sharing across background tasks.

```rust
pub struct Platform {
    pub os: OsFamily,
    pub system: Arc<dyn SystemInfo>,
    pub packages: Arc<dyn PackageManager>,
    pub processes: Arc<dyn ProcessManager>,
    pub security: Arc<dyn SecurityAuditor>,
    pub fail2ban: Arc<dyn Fail2BanManager>,
    pub gateway: Arc<dyn GatewayManager>,
    pub tunnel: Arc<dyn TunnelManager>,
    pub firewall: Arc<dyn FirewallManager>,
    pub docker: Arc<dyn DockerManager>,
    pub wasm_cloud: Arc<dyn WasmCloudManager>,
    pub ssh: Arc<dyn SshKeyManager>,
    pub services: Arc<dyn ServiceManager>,
    pub users: Arc<dyn UserManager>,
    pub nats: Arc<NatsManager>,
    pub workloads: Arc<dyn ManagedWorkloadManager>,
}
```

Every subsystem is behind an `async_trait` trait with async methods. Implementations shell out to CLI tools (`apt`, `dnf`, `docker`, `cloudflared`, `wash`, etc.) via `tokio::process::Command`. No external daemons, no embedded engines — Postlab is a CLI orchestrator, not a platform.

### `db/` — Persistence layer

SQLite via `sqlx`. The database is created at `~/.postlab/data.db` (configurable with `--database`). Tables are created in `db/mod.rs` with `CREATE TABLE IF NOT EXISTS`. Historical migrations exist in `migrations/` as append-only SQL files (001 created the original multi-server schema, 002 removed server references for the local-execution model).

Current schema:
- `audit_log` — records every package install/remove with success/failure and timestamp
- `deployments` — Git-based deployment records (repo, path, type, status)

### `tui/` — Presentation layer

#### App state (`app.rs`)

`App` is the central state machine. It holds:
- Current screen and sub-tab enums
- Per-screen state structs (e.g., `DashboardState`, `PackagesState`, `DockerState`)
- A `tokio::sync::mpsc` channel pair (`task_tx`/`task_rx`) for background task results
- The `Platform` (in `Arc`) and `SqlitePool`

Every screen's state is eagerly allocated at startup (no lazy loading). Each state struct holds list/table cursors, filter strings, input mode flags, and cached data.

#### Event loop (`mod.rs`)

The main loop runs at 250ms ticks:
1. **Draw** — full terminal redraw: nav bar, screen content, status bar, optional confirm dialog
2. **Poll input** — crossterm event polling with the tick duration as timeout
3. **Process task results** — drain the `task_rx` channel and update state
4. **Tick** — periodic data refresh (CPU, memory, processes, NATS health, etc.)

The `needs_login` flag suspends the TUI to run `cloudflared tunnel login` in the foreground, then resumes.

#### Input dispatch (`events.rs`)

`handle_key` implements a priority chain:
1. **Confirm dialog** — `y`/`Enter` confirms, anything else cancels
2. **Text input mode** — if any screen is in `Editing`/`SettingPassword`/`AddingDomain` mode, all keys are consumed for text entry
3. **Global keys** — `q` (quit), `1-8` (screen switching), `Tab`/`Shift+Tab`
4. **Screen-specific keys** — dispatched to per-screen handlers

`handle_click` translates mouse events to tab switching and list/table row selection.

#### Background tasks

All I/O is offloaded to `tokio::spawn` tasks. The app spawns tasks via helper methods (`spawn_load_packages`, `spawn_install`, `spawn_security_scan`, etc.) that clone the `Platform` Arc and the `task_tx` sender. Results are sent back as `TaskResult` enum variants and processed synchronously in the draw loop.

```
User Action → spawn_* { Platform.clone(), tx.clone() }
  → tokio::spawn { platform.packages.install(...).await; tx.send(result) }
  → Event loop drains task_rx → App state updated → next draw
```

## Data Flow

1. **Startup**: `main.rs` checks root, parses CLI args, initializes DB, calls `core::platform::detect()`, then either runs a one-shot command or hands off to `tui::run()`.
2. **TUI lifecycle**: `tui::run()` creates the `App` with the `Platform` in `Arc`, sets the initial screen to Dashboard, and enters the render/event loop.
3. **Screen navigation**: global keys `1-8` or `Tab` switch screens via `app.set_screen()`, which triggers lazy data loading for the target screen.
4. **Sub-tab navigation**: `←`/`→` or `h`/`l` cycle sub-tabs within screens. Each tab switch triggers its own data load.
5. **Operations**: destructive actions (install, remove, kill, stop, delete) go through a confirm dialog. Confirmed actions spawn background tasks that update state via the `TaskResult` channel.

## Screen Architecture

Each screen in `tui/screens/` exports a `render` function that takes `&mut Frame` and `&App`. The function checks the current screen/tab and renders the appropriate widgets. Screens are pure functions — they read from `App` state but never mutate it directly. All mutations happen in `events.rs` handlers or via `TaskResult` processing.

| Screen | File | Key data loaded |
|--------|------|----------------|
| Dashboard | `dashboard.rs` | OS info, CPU %, memory, disk, processes, resource history |
| Packages | `packages.rs` | Installed packages, search results, curated list, operation queue |
| Security | `security.rs` | Findings, firewall rules, port status, SSH keys, Fail2Ban jails |
| Networking | `networking.rs` | Caddy routes, Cloudflare tunnels, ingress config |
| Docker | `docker.rs` | Containers, images, Compose stacks, workloads, managed services |
| wasmCloud | `wasmcloud.rs` | Hosts, components, apps, NATS health, inspector |
| Automation | `automation.rs` | Pi Agent daemon, channels, cron, memory, config, permissions, skills, auth, logs |
| System | `system.rs` | Ghost processes, janitor, services, users, swap |

## Platform Detection

At startup, `OsFamily::detect()` checks for package managers on the PATH in order: `apt-get` → `dnf`/`yum` → `pacman` → `brew`. This determines the OS family, which gates security checks and fix strategies.

Individual subsystem detection is independent:
- **Package manager**: `apt` → `dnf` → `pacman` → `brew` (must find one, or startup fails)
- **Firewall**: `ufw` → `firewall-cmd` → `pfctl` → NoneManager
- **Container engine**: `docker` → `podman` (DockerManager)
- **Service manager**: `systemd` → `launchd` (macOS)
- **Workload backend**: Podman Quadlet (preferred) → Docker Compose + systemd (Linux only)

## Model Types

`core/models.rs` defines all shared data types used across the application. All types implement `Serialize`, `Deserialize`, `Debug`, and `Clone`. Key types:

- **System**: `OsInfo`, `MemInfo`, `DiskInfo`, `NetStats`
- **Packages**: `Package` (name, version, description, installed)
- **Processes**: `ProcessEntry` (pid, name, cpu_pct, mem_bytes, user, status)
- **Security**: `SecurityFinding` (id, title, severity, description, fix), `Severity` enum, `JailedIp`
- **Networking**: `Route`, `Tunnel`, `FirewallRule`, `TunnelRoute`
- **Docker**: `DockerContainer`, `DockerImage`, `DockerComposeService`, `ManagedDockerService`, `ManagedWorkload`, `ManagedWorkloadSpec`
- **wasmCloud**: `WasmCloudHost`, `WasmCloudComponent`, `WasmCloudApp`, `WasmCloudLink`
- **System**: `GhostProcess`, `GhostReason`, `SwapStatus`, `SwapEntry`, `UserInfo`
- **Deployments**: `Deployment`, `DeploymentType`, `DeploymentStatus`

## Input Mode System

The TUI uses an `InputMode` enum to manage text entry:

```rust
pub enum InputMode {
    Normal,           // Key dispatches to handlers
    Editing,          // Text input mode (filters, forms, search)
    SettingPassword,  // Password entry (masked)
    AddingDomain,     // Tunnel domain entry
    EditingIngress,   // Ingress entry editing
}
```

When any mode other than `Normal` is active, the event loop routes all keystrokes to the relevant input handler instead of the global dispatcher. This prevents accidental screen switches or quits while typing.

## Confirm Dialog

Destructive actions use a confirm dialog rendered as a centered popup. The `ConfirmAction` enum encodes the action type and its parameters. `y`/`Enter` confirms, any other key cancels. The dialog is rendered by `render_confirm_dialog` in the main draw loop, overlaid on top of the current screen content.

## Database

SQLite via `sqlx` with `SqlitePool`. The database file is created at `~/.postlab/data.db` by default. The `init_db` function creates tables with `IF NOT EXISTS` guards. The `audit_log` table records every package install/remove with timestamp and success/failure. The `deployments` table tracks Git-based deployments.

Historical migrations in `migrations/` are append-only — they document the schema evolution but are not executed at runtime. The current schema is maintained directly in `db/mod.rs`.

## Build System

The Makefile provides:

| Target | Description |
|--------|-------------|
| `build` | Dev build |
| `check` | `cargo check` + `cargo clippy` with `-D warnings` |
| `test` | Unit tests |
| `run` | Dev build + run |
| `build-release` | Stripped, LTO release binary |
| `build-linux` | Cross-compile to x86_64 Linux via `cargo-zigbuild` |
| `dev` | Watch + auto-restart via `cargo-watch` |
| `install` | Release build → `/usr/local/bin/postlab` |

The release profile uses `opt-level = "z"`, `strip = true`, `lto = true`, and `codegen-units = 1` for minimal binary size (~8-15 MB).

## Key Design Decisions

1. **No embedded engines** — Postlab shells out to existing CLIs. It does not embed Docker, Caddy, cloudflared, or wasmCloud. This keeps the binary small and avoids version coupling.

2. **Eager state allocation** — All screen states are allocated at startup rather than lazily. This simplifies the code (no `Option` unwrapping) and the memory overhead is negligible for a server management tool.

3. **Trait-based platform abstraction** — Every subsystem is behind an `async_trait` trait, enabling runtime detection and platform-specific implementations without conditional compilation.

4. **Single-owner state** — `App` owns all state. Screens are pure render functions. Mutations only happen in event handlers and task result processing. This eliminates shared mutable state bugs.

5. **Streaming install output** — Package managers proxy stdout/stderr line-by-line through `tokio::sync::mpsc` channels, giving real-time feedback in the operation queue.

6. **Config file backups** — Every destructive config change creates a timestamped `.bak` file before modifying the original.

7. **Root requirement** — Postlab checks `nix::unistd::Uid::effective().is_root()` at startup and exits if not root. This is required for package management, service control, and config file writes.