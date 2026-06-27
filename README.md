# Postlab

<div align="center">
<img width="600" alt="Postlab Logo High Resolution" src="https://github.com/user-attachments/assets/edbb6950-8aef-4c5d-9f95-e0e0e51074c5" />
  

  <p>
    <b>Interactive bare metal server manager — runs directly on the machine it manages.</b>
  </p>

  <p>
    <img src="https://img.shields.io/badge/version-0.2.0-blue.svg" alt="Version 0.2.0">
    <img src="https://img.shields.io/badge/license-Apache--2.0-green.svg" alt="License Apache-2.0">
    <img src="https://github.com/rifkyputra/postlab/actions/workflows/build.yml/badge.svg" alt="Build Status">
  </p>
</div>

Single binary. Low memory. Cross-platform (Linux + macOS).

> [!IMPORTANT]  
> **Postlab must run as root** to manage packages, services, and system configuration files.

---

## Features

| Screen | Tabs / Sub-features | What it does |
| ------ | ------------------- | ------------ |
| **1. Dashboard** | Overview, Processes, Resources | Live hostname, OS, uptime, CPU cores, memory, disk gauges, and performance history. Process list with sort and kill. |
| **2. Packages** | Installed, Search, Quick Install, Queue | Install / remove / upgrade packages; curated quick-install list; background operation queue. |
| **3. Security** | Findings, Firewall, Ports, SSH, Fail2Ban | SSH/ASLR audits with one-click fixes; UFW / firewalld / pf management; external port checker; authorized_keys manager; Fail2Ban list / ban / unban. |
| **4. Networking** | Gateway, Tunnel, Tailscale | **Gateway:** Caddy installation and route management (domain → port) with automatic TLS. **Tunnel:** Cloudflare tunnel creation, route management, and ingress configuration. **Tailscale:** Install Tailscale, view VPN status and peers, bring up/down the connection. |
| **5. Docker** | Containers, Images, Compose, Workloads, Managed | Manage Docker or Podman lifecycle, view image sizes, control Compose stacks, host-managed workloads, and one-click dev services (PostgreSQL, Redis, RabbitMQ, etc.). |
| **6. wasmCloud** | Hosts, Components, Apps, Inspector | Manage wasmCloud lattices, host nodes, components, and applications. NATS backbone health and interactive inspector. |
| **7. Agent** | Chat, Tools, Tasks, Status, Sessions, Config, Auth, Skills, Library, Logs | Interactive chat with pi agent; tool execution log; background task scheduler; install/update management; session browser; config/auth viewer; skill library with one-click install; live log tail. |
| **8. System** | Ghosts, Janitor, Services, Users, Swap | Ghost process hunter; package-cache cleanup; systemd / launchd service control; Unix user CRUD and sudoers; swap file creation, resize, and enable/disable. |
| **9. Projects** | Projects, New, Clone, Settings | Browse local projects by last modified time; scaffold a new app via create-better-t-stack; clone a GitHub repo by shorthand or URL; configure the projects directory. |

All operations are **non-blocking** — the TUI stays responsive while background tasks (like package installations) run.
Every destructive change to config files creates a timestamped `.bak` backup first.

### Docker / Workloads Disclaimers

> [!WARNING]
> The `Workloads` tab is a **system-level** feature. In v1 it is available only on **Linux hosts with `systemd`** and is intentionally unavailable on macOS or non-`systemd` environments.

> [!IMPORTANT]
> Postlab manages only **Postlab-owned canonical workload files** in the `Workloads` tab. It does **not** import, rewrite, or assume ownership of arbitrary existing Quadlet files, Compose projects, or hand-written systemd units.

> [!NOTE]
> Backend behavior is engine-specific by design:
> - **Podman** workloads are rendered as Quadlet `.container` units.
> - **Docker** workloads are rendered as a generated single-service `compose.yml` plus a generated `systemd` unit.
> - Existing ad-hoc Compose stacks should continue to be managed through the regular `Compose` tab, not `Workloads`.
> - The **Managed** tab provides curated one-click dev containers (PostgreSQL, Redis, MySQL, MongoDB, Elasticsearch, MinIO, MailHog, RabbitMQ) separate from Workloads.

> [!CAUTION]
> Workloads in v1 are **single-service only**. They are meant for durable host-managed services, not multi-service application graphs, cluster scheduling, or generic orchestration import/export.

---

## Quick Start

### Installation

**One-liner (Linux x86_64 / macOS arm64):**

```bash
curl -fsSL https://raw.githubusercontent.com/rifkyputra/postlab/main/install.sh | bash
```

The script detects your OS and architecture, downloads the right binary, and installs it to `/usr/local/bin/postlab`. Running it again upgrades an existing installation.

**Custom install path:**

```bash
curl -fsSL https://raw.githubusercontent.com/rifkyputra/postlab/main/install.sh | DEST=~/.local/bin/postlab bash
```

**Build from source:**

```bash
git clone https://github.com/rifkyputra/postlab.git
cd postlab
make install   # builds release binary → /usr/local/bin/postlab
```

### Usage

```bash
# Launch interactive TUI (default)
sudo postlab

# One-shot commands (no TUI)
sudo postlab info    # Print system summary
sudo postlab list    # List installed packages

# Custom SQLite database path (default: ~/.postlab/data.db)
sudo postlab --database /var/lib/postlab/data.db
```

---

## Keybindings

### Navigation

| Key | Action |
| --- | ------ |
| `1`–`9` | Switch screens |
| `a` | Jump to Agent (global, except on Agent screen) |
| `s` | Jump to System (global, except on System screen) |
| `Tab` / `Shift+Tab` | Next / previous screen |
| `H` / `L` or `←` `→` | Switch tabs within a screen |
| `↑` `↓` | Navigate lists or tables |
| `Enter` | Confirm / execute / drill-down |
| `q` | Quit |

### Actions

| Key | Context | Action |
| --- | ------- | ------ |
| `Space` | Lists | Toggle selection |
| `/` | Packages, Services, Config | Search / filter |
| `r` / `R` | Global | Refresh current screen/tab data |
| `k` | Processes / Ghosts | Kill selected process |
| `a` | Gateway / Tunnel / Workloads | Add route / create tunnel / create workload |
| `D` | Gateway / Tunnel | Delete selected route / ingress entry |
| `f` | Tunnel | Toggle focus between Tunnels and Ingress panels |
| `s` / `k` / `r` | System → Services | Start / stop / restart selected unit |
| `e` / `d` | System → Services | Enable / disable selected unit |

---

## Pi Agent Integration

The **Agent** screen manages pi agent directly from Postlab:

- Install and update pi agent from the Status tab
- Chat interactively with pi agent and watch tool executions in real time
- Schedule background agent jobs with configurable intervals (30m–24h)
- Browse past sessions, view config/auth, and manage installed skills
- Install skills from the curated Library with a single keypress
- Tail the most recent session log

Postlab shells out to the pi agent CLI via a JSONL-over-stdin/stdout RPC protocol — it does not embed pi agent source code. The agent process is spawned under the original user's UID/GID (via `$SUDO_UID`/`$SUDO_GID`) so it never runs as root.

---

## Architecture

Postlab is built with a clean separation between the core logic and the TUI. The `core/` layer can be used independently (e.g., by an API or a future web interface).

> 📖 **TUI layout & mouse capture** — see [`docs/tui.md`](docs/tui.md) for screen layout, tab bar positions, and mouse click dispatch.
> 📖 **Full architecture** — see [`docs/architecture.md`](docs/architecture.md) for platform detection, data flow, background tasks, and design decisions.

```
cli/src/
├── main.rs                  # clap entry: info | list | tui (default)
├── core/
│   ├── platform.rs          # Platform { system, packages, processes, ... }
│   │                        # detect() — auto-selects right impls at runtime
│   ├── models.rs            # Shared data types
│   ├── system/              # SystemInfo trait + sysinfo impl
│   ├── packages/            # PackageManager trait + apt / dnf / pacman / brew
│   ├── processes/           # ProcessManager trait + sysinfo impl
│   ├── security/            # SecurityAuditor trait + SSH/ASLR checks
│   ├── firewall/            # FirewallManager trait + ufw / firewalld / pf
│   ├── portcheck/           # External port reachability checker
│   ├── ssh/                 # SshKeyManager trait + authorized_keys / ssh-keygen
│   ├── services/            # ServiceManager trait + systemd / launchd
│   ├── users/               # Unix user management
│   ├── docker/              # DockerManager trait + docker / podman CLI
│   ├── workloads/           # Host-managed single-service workloads
│   ├── wasm_cloud/          # wasmCloud CLI management
│   ├── nats/                # NATS backbone health for wasmCloud
│   ├── ghost/               # Ghost service detection logic
│   ├── gateway/             # GatewayManager trait + Caddy impl
│   ├── tunnel/              # TunnelManager trait + cloudflared impl
│   ├── tailscale/           # Tailscale CLI integration
│   ├── deploy/              # Git-based deployment runner
│   ├── projects/            # Local project browser and scaffolder
│   └── pi_agent/            # Pi Agent CLI integration
├── db/
│   ├── mod.rs               # init_db (SQLite, auto-create ~/.postlab/data.db)
│   ├── audit.rs             # Log actions for audit history
│   ├── deployments.rs       # Deployment record CRUD
│   └── agent_tasks.rs       # Scheduled agent task CRUD
└── tui/
    ├── mod.rs               # Terminal init + main event loop
    ├── app.rs               # App state machine and background task management
    ├── events.rs            # Keyboard dispatch (global + screen-specific)
    └── screens/             # UI for all 9 top-level screens
```

---

## Support

### Package Managers

Detected automatically at startup:

- **Debian / Ubuntu**: `apt`
- **Fedora / RHEL**: `dnf` / `yum`
- **Arch**: `pacman`
- **macOS**: `brew`

### Container Engines

Detected automatically — prefers `docker`, falls back to `podman`.

### Firewalls

Detected automatically at startup:

- **Debian / Ubuntu**: `ufw`
- **Fedora / RHEL / Arch**: `firewalld`
- **macOS**: `pf`

### Security Hardening Audits

| Check | Severity | Action |
| ----- | -------- | ------ |
| SSH root login enabled | Critical | One-click fix |
| SSH password auth enabled | High | One-click fix |
| Firewall (ufw / firewalld / pf) inactive | High | One-click fix |
| ASLR not fully enabled | Medium | One-click fix |
| Auto-updates not configured | Low | One-click fix |

Every fix creates a `.bak.<timestamp>` copy of the config file first, e.g., `/etc/ssh/sshd_config.bak.20260303T142031`.

---

## Roadmap

- [x] **Docker** — Containers, images, Compose, managed dev services, and host workloads.
- [x] **wasmCloud** — Host, component, and app management with NATS health and inspector.
- [x] **SSH Keys** — Interactive authorized_keys manager.
- [x] **Firewall** — UFW / firewalld / pf rule management.
- [x] **Ghost Hunter** — Detect abandoned services and processes.
- [x] **Services** — systemd / launchd start, stop, restart, enable, disable.
- [x] **Users & Swap** — Unix account management and swap file control.
- [x] **Pi Agent** — Full Agent screen: chat, tasks, sessions, skills library, config, auth, and logs.
- [x] **Tailscale** — VPN status, peer list, and connection control.
- [x] **Projects** — Browse, scaffold, and clone local projects.
- [ ] **Snapshots** — Btrfs/ZFS snapshot management.
- [ ] **Web API** (axum) — Expose `core::Platform` over HTTP.

---

## Development

### Requirements

- Rust 1.75+
- SQLite

### Local Build

```bash
make build   # Dev build
make release # Optimized release binary (~8–15 MB)
make run     # Build and run interactive mode
```

---

## License

This project is licensed under the Apache License, Version 2.0.
See the [LICENSE](LICENSE) file for details.
