<div align="center">
<pre>
██████╗  ██████╗ ███████╗████████╗██╗      █████╗ ██████╗ 
██╔══██╗██╔═══██╗██╔════╝╚══██╔══╝██║     ██╔══██╗██╔══██╗
██████╔╝██║   ██║███████╗   ██║   ██║     ███████║██████╔╝
██╔═══╝ ██║   ██║╚════██║   ██║   ██║     ██╔══██║██╔══██╗
██║     ╚██████╔╝███████║   ██║   ███████╗██║  ██║██████╔╝
╚═╝      ╚═════╝ ╚══════╝   ╚═╝   ╚══════╝╚═╝  ╚═╝╚═════╝ 
</pre>

  <p>
    <b>Interactive bare metal server manager — runs directly on the machine it manages.</b>
  </p>

  <p>
    <img src="https://img.shields.io/badge/version-0.3.0-blue.svg" alt="Version 0.3.0">
    <img src="https://img.shields.io/badge/license-Apache--2.0-green.svg" alt="License Apache-2.0">
    <img src="https://github.com/rifkyputra/postlab/actions/workflows/build.yml/badge.svg" alt="Build Status">
  </p>
</div>

Single binary. Low memory. Cross-platform (Linux + macOS).

> [!IMPORTANT]  
> **Postlab must run as root** to manage packages, services, and system configuration files.

---

## Demo

<div align="center">
  <img src="demo.gif" alt="Postlab TUI demo — touring the Dashboard, Packages, Security, Networking, Docker, and Agent screens" width="900">
</div>

> Regenerate the GIF with [`vhs`](https://github.com/charmbracelet/vhs): `vhs demo.tape`.

---

## Features

| Screen | Tabs / Sub-features | What it does |
| ------ | ------------------- | ------------ |
| **1. Dashboard** | Overview, Processes, Resources | Live hostname, OS, uptime, CPU cores, memory, disk gauges, and performance history. Process list with sort and kill. |
| **2. Packages** | Installed, Search, Quick Install, Queue, Updates | Install / remove / upgrade packages; curated quick-install list; background operation queue; view available upgrades with version deltas and security flagging. |
| **3. Security** | Findings, Firewall, Ports, SSH, Fail2Ban | SSH/ASLR audits with one-click fixes; UFW / firewalld / pf management; external port checker; authorized_keys manager; Fail2Ban list / ban / unban. |
| **4. Networking** | Gateway, Tunnel, Tailscale | **Gateway:** Caddy installation and route management (domain → port) with automatic TLS. **Tunnel:** Cloudflare tunnel creation, route management, and ingress configuration. **Tailscale:** Install Tailscale, view VPN status and peers, bring up/down the connection. |
| **5. Docker** | Containers, Images, Compose, Workloads, Managed | Manage Docker or Podman lifecycle, view image sizes, control Compose stacks, host-managed workloads, and one-click dev services (PostgreSQL, Redis, RabbitMQ, etc.). |
| **6. wasmCloud** | Hosts, Components, Apps, Inspector | Manage wasmCloud lattices, host nodes, components, and applications. NATS backbone health and interactive inspector. |
| **7. Agent** | Chat, Tools, Tasks, Status, Sessions, Config, Auth, Skills, Library, Logs | Interactive chat with pi agent; tool execution log; background task scheduler; install/update management; session browser; config/auth viewer; skill library with one-click install; live log tail. |
| **8. System** | Ghosts, Janitor, Services, Users, Swap, Storage | Ghost process hunter; package-cache cleanup; systemd / launchd service control; Unix user CRUD and sudoers; swap file creation, resize, and enable/disable; filesystem browser with mount/unmount, physical disk SMART health, and /etc/fstab viewer. |
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

**One-liner (Linux x86_64 / Linux arm64 / macOS arm64):**

```bash
curl -fsSL https://raw.githubusercontent.com/rifkyputra/postlab/main/install.sh | bash
```

The script detects your OS and architecture, downloads the matching binary from the latest [GitHub release](https://github.com/rifkyputra/postlab/releases), and installs it to `/usr/local/bin/postlab`. Running it again upgrades an existing installation.

**Custom install path:**

```bash
curl -fsSL https://raw.githubusercontent.com/rifkyputra/postlab/main/install.sh | DEST=~/.local/bin/postlab bash
```

**Pin a specific version:**

```bash
curl -fsSL https://raw.githubusercontent.com/rifkyputra/postlab/main/install.sh | VERSION=v0.3.0 bash
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
sudo postlab info                       # System summary
sudo postlab list                       # Installed packages
sudo postlab system info                # OS, kernel, CPU, memory, uptime
sudo postlab system cpu                 # Per-core CPU percentages
sudo postlab system mem                 # Memory usage
sudo postlab system disks               # Disk usage by mount point
sudo postlab system net                 # Network RX/TX bytes
sudo postlab system swap status         # Swap status and entries
sudo postlab system swap create PATH M  # Create swap file (size in MiB)
sudo postlab system swap delete PATH    # Remove swap file
sudo postlab packages list              # Installed packages
sudo postlab packages search QUERY      # Search package index
sudo postlab packages install NAME      # Install a package
sudo postlab packages remove NAME       # Remove a package
sudo postlab packages upgrade           # Upgrade all packages
sudo postlab packages cache-clean       # Clean package caches
sudo postlab packages list-upgradable   # Available updates with security flags
sudo postlab processes list             # Running processes (sort: -s cpu|mem|pid|name)
sudo postlab processes kill PID         # Kill process (SIGTERM)
sudo postlab processes kill PID -f      # Kill process (SIGKILL)
sudo postlab services list              # Systemd services
sudo postlab services start NAME        # Start a service
sudo postlab services stop NAME         # Stop a service
sudo postlab services restart NAME      # Restart a service
sudo postlab services enable NAME       # Enable at boot
sudo postlab services disable NAME      # Disable at boot
sudo postlab storage list               # Filesystems + physical disks (SMART)

# Custom SQLite database path (default: ~/.postlab/data.db)
sudo postlab --database /var/lib/postlab/data.db
```

---

## Keybindings

### Navigation

| Key | Action |
| --- | ------ |
| `1`–`9` | Switch screens |
| `a` | Open Agent overlay (global) |
| `s` | Jump to System (global, except on System screen) |
| `Tab` / `Shift+Tab` | Next / previous screen |
| `H` / `L` or `←` `→` | Switch tabs within a screen |
| `↑` `↓` | Navigate lists or tables |
| `Enter` | Confirm / execute / drill-down |
| `Esc` | Clear filter / cancel input / return focus |
| `q` | Quit |

### Actions

| Key | Context | Action |
| --- | ------- | ------ |
| `Space` | Lists | Toggle selection |
| `r` / `R` | Most screens | Refresh current data |
| `/` | Packages, Services, Agent Config | Search / filter |
| `Esc` | Search/filter mode | Clear filter |
| **Dashboard → Processes** |||
| `c` / `m` / `p` | Process list | Sort by CPU / memory / PID |
| `k` | Process list | Kill selected process (with confirm) |
| **Packages** |||
| `d` | Installed tab | Remove selected package(s) |
| `i` | Search tab | Install selected package(s) |
| `u` | Updates tab | Install selected update(s) |
| `U` | Updates tab | Upgrade all packages |
| **Security → Findings** |||
| `s` | Findings tab | Run security scan |
| `Enter` | Findings tab | Apply selected fix (with confirm) |
| **Security → Firewall** |||
| `a` | Firewall tab | Add new firewall rule |
| `D` | Firewall tab | Delete selected rule |
| **Security → SSH** |||
| `g` | SSH tab | Generate new key pair |
| **Security → Fail2Ban** |||
| `f` | Jailed list | Forgive (unban) selected IP |
| `b` | Jailed list | Banish (permanent block) selected IP |
| **Networking → Gateway** |||
| `a` | Gateway tab | Add route (domain → port) |
| `D` | Gateway tab | Delete selected route |
| `i` | Gateway tab | Install Caddy |
| **Networking → Tunnel** |||
| `a` | Tunnel tab | Create new tunnel |
| `d` | Tunnel tab | Add domain to selected tunnel config |
| `D` | Ingress panel | Delete selected ingress entry |
| `e` | Ingress panel | Edit selected ingress service |
| `f` | Tunnel tab | Toggle focus between Tunnels/Ingress |
| `s` | Tunnel tab | Install cloudflared system service |
| `c` | Tunnel tab | Refresh config + service status |
| `i` | Tunnel tab | Install cloudflared |
| `Enter` | Tunnel list | Select active tunnel (shows config + status) |
| **Networking → Tailscale** |||
| `i` | Tailscale tab | Install Tailscale |
| `u` | Tailscale tab | Bring Tailscale up |
| `d` | Tailscale tab | Take Tailscale down |
| **Docker** |||
| `s` | Containers tab | Start selected container |
| `k` | Containers tab | Stop selected container (with confirm) |
| `D` | Containers / Images | Remove container or image (with confirm) |
| **wasmCloud** |||
| `i` | wasmCloud tab | Install wash CLI |
| `Enter` | Inspector | Inspect component (paste WIT path) |
| **System → Ghosts** |||
| `k` | Ghost list | Kill selected ghost process |
| `r` | Ghost list | Re-scan for ghost processes |
| **System → Services** |||
| `s` / `k` / `r` | Service list | Start / stop / restart selected unit |
| `e` / `d` | Service list | Enable / disable selected unit |
| `c` | Janitor tab | Clean package manager caches |
| **Projects** |||
| `p` | Projects tab | git pull in selected project |
| `c` | Clone tab | Clone (after entering repo URL) |
| `s` | New tab | Open stack configurator popup |

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
├── main.rs                  # clap entry: 8 subcommands + TUI (default)
├── core/
│   ├── platform.rs          # Platform { system, packages, processes, ... }
│   │                        # detect() — auto-selects right impls at runtime
│   ├── models.rs            # Shared data types (30+ structs/enums)
│   ├── system/              # SystemInfo trait + sysinfo impl (swap, fstab)
│   ├── packages/            # PackageManager trait + apt / dnf / pacman / brew
│   ├── processes/           # ProcessManager trait + sysinfo impl
│   ├── security/            # SecurityAuditor trait + SSH/ASLR/firewall checks
│   ├── firewall/            # FirewallManager trait + ufw / firewalld / pf
│   ├── portcheck/           # External port reachability checker (portchecker.co)
│   ├── ssh/                 # SshKeyManager trait + authorized_keys / ssh-keygen
│   ├── services/            # ServiceManager trait + systemd / launchd
│   ├── users/               # Unix user CRUD + group management
│   ├── storage/             # lsblk + SMART health, mount/umount, fstab viewer
│   ├── docker/              # DockerManager trait + docker / podman CLI
│   ├── workloads/           # Host-managed single-service workloads
│   ├── wasm_cloud/          # wasmCloud CLI management (wash)
│   ├── nats/                # NATS backbone health for wasmCloud
│   ├── ghost/               # Ghost/zombie/orphan process detection
│   ├── gateway/             # GatewayManager trait + Caddy impl
│   ├── tunnel/              # TunnelManager trait + cloudflared impl
│   ├── tailscale/           # Tailscale VPN status, peers, up/down
│   ├── deploy/              # Git-based deployment runner + detector
│   ├── projects/            # Local project browser, scaffold (create-better-t-stack), clone
│   └── pi_agent/            # Pi Agent CLI integration (install, chat, sessions, skills)
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

- [ ] **Snapshots** — Btrfs/ZFS snapshot management.
- [ ] **Web API** (axum) — Expose `core::Platform` over HTTP.
- [ ] **Multi-service workloads** — Compose-level service graphs in the Workloads tab.
- [ ] **SSH config hardening** — Full sshd_config audit, key rotation, and MFA setup.

---

## Changelog

### v0.3.0 — 2026-06-28

First feature release since v0.2.0. Highlights:

- **User management** — list, create, and delete users with password and sudoers handling.
- **WasmCloud + NATS** — service management, backbone provisioning, and a component inspector.
- **Managed Docker services** — managed workloads, Podman support, and Firewalld/pf managers.
- **Pi Agent** (formerly Zeroclaw) — chat, tools, tasks, sessions, skills, auth, and logs.
- **Networking** — Gateway (Caddy), Tunnel (Cloudflare), and Tailscale tabs.
- **Projects** and **Automation/Agent** screens, plus swap management, a Storage tab, and an Updates tab.
- Installer now pulls per-platform tarballs from GitHub Releases (Linux x86_64/arm64, macOS arm64).

See [CHANGELOG.md](CHANGELOG.md) for the full history and the [v0.3.0 release](https://github.com/rifkyputra/postlab/releases/tag/v0.3.0) for downloads.

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
