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
| **4. Networking** | Gateway, Tunnel | **Gateway:** Caddy installation and route management (domain → port) with automatic TLS. **Tunnel:** Cloudflare tunnel creation, route management, and ingress configuration. |
| **5. Docker** | Containers, Images, Compose, Workloads, Managed | Manage Docker or Podman lifecycle, view image sizes, control Compose stacks, host-managed workloads, and one-click dev services (PostgreSQL, Redis, RabbitMQ, etc.). |
| **6. wasmCloud** | Hosts, Components, Apps, Inspector | Manage wasmCloud lattices, host nodes, components, and applications. NATS backbone health and interactive inspector. |
| **7. Automation** | Overview, Channels, Cron, Memory, Config, Easy Config, Permissions, Skills, Auth, Logs | Install and manage [ZeroClaw](https://github.com/zeroclaw-labs/zeroclaw): daemon control, channels, cron jobs, agent memory, TOML config editing, permissions, skills, auth profiles, and live logs. |
| **8. System** | Ghosts, Janitor, Services, Users, Swap | Ghost process hunter; package-cache cleanup; systemd / launchd service control; Unix user CRUD and sudoers; swap file creation, resize, and enable/disable. |

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
| `1`–`8` | Switch screens |
| `a` | Jump to Automation (global, except on Automation screen) |
| `s` | Jump to System (global, except on Automation screen) |
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
| `r` / `R` | Global | Refresh current screen/tab data (context-specific) |
| `k` | Processes / Ghosts | Kill selected process |
| `a` | Gateway / Tunnel / Workloads | Add route / create tunnel / create workload |
| `D` | Gateway / Tunnel | Delete selected route / ingress entry |
| `f` | Tunnel | Toggle focus between Tunnels and Ingress panels |
| — | Security → Findings | Auto-scans on first visit; re-scan after applying fixes |
| `s` / `k` / `r` | System → Services | Start / stop / restart selected unit |
| `e` / `d` | System → Services | Enable / disable selected unit |

---

## ZeroClaw Integration

The **Automation** screen manages [ZeroClaw](https://github.com/zeroclaw-labs/zeroclaw) directly from Postlab:

- Install zeroclaw and its systemd service from the Overview tab
- Start / stop the daemon, run doctor, and check for updates
- Manage channels, cron schedules, agent memory, skills, and auth profiles
- Edit raw TOML config or use guided Easy Config / Permissions editors
- Tail daemon logs with optional follow mode

Implementation lives in `cli/src/core/zeroclaw/` and `cli/src/tui/screens/automation.rs`. Postlab shells out to the `zeroclaw` CLI — it does not embed zeroclaw source code.

---

## Architecture

Postlab is built with a clean separation between the core logic and the TUI. The `core/` layer can be used independently (e.g., by an API or a future web interface).

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
│   └── zeroclaw/            # ZeroClaw CLI integration
├── db/
│   ├── mod.rs               # init_db (SQLite, auto-create ~/.postlab/data.db)
│   └── audit.rs             # Log actions for audit history
└── tui/
    ├── mod.rs               # Terminal init + main event loop
    ├── app.rs               # App state machine and background task management
    ├── events.rs            # Keyboard dispatch (global + screen-specific)
    └── screens/             # UI for all 8 top-level screens
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
- [x] **ZeroClaw** — Full Automation screen for agent lifecycle and config.
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
