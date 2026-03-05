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

| Screen           | Tabs / Sub-features                      | What it does                                                                                                 |
| ---------------- | ---------------------------------------- | ------------------------------------------------------------------------------------------------------------ |
| **1. Dashboard** | Overview, Processes, Resources           | Live Hostname, OS, uptime, CPU cores, memory, disk gauges, and performance history.                          |
| **2. Packages**  | Installed, Search, Quick, Queue          | Install / remove / upgrade packages; curated quick-install list; background operation queue.                 |
| **3. Security**  | Findings, Firewall, Ports, SSH, Fail2Ban | SSH/ASLR audits; UFW/Firewall management; external port checker; authorized_keys manager; Fail2Ban list/ban. |
| **4. Gateway**   | Caddy                                    | Caddy installation and route management (domain → port) with automatic TLS.                                  |
| **5. Tunnel**    | Cloudflare                               | Cloudflare tunnel creation, route management, and ingress configuration.                                     |
| **6. Docker**    | Containers, Images, Compose              | Manage Docker lifecycle, view image sizes, and control Docker Compose stacks.                                |
| **7. wasmCloud** | Hosts, Components, Apps                  | Manage wasmCloud lattices, host nodes, components, and applications.                                         |
| **8. Ghosts**    | Services Hunter                          | Identifies "ghost" services or abandoned processes that may be using resources or ports.                     |

All operations are **non-blocking** — the TUI stays responsive while background tasks (like package installations) run.
Every destructive change to config files creates a timestamped `.bak` backup first.

---

## Quick Start

### Installation

```bash
# Build and install to /usr/local/bin
make install
```

### Usage

```bash
# Launch interactive TUI (default)
sudo postlab

# One-shot commands (no TUI)
sudo postlab info    # Print system summary
sudo postlab list    # Print installed packages
```

---

## Keybindings

### Navigation

| Key                  | Action                         |
| -------------------- | ------------------------------ |
| `1`–`8`              | Switch screens                 |
| `Tab` / `Shift+Tab`  | Next / previous screen         |
| `H` / `L` or `←` `→` | Switch tabs within a screen    |
| `↑` `↓`              | Navigate lists or tables       |
| `Enter`              | Confirm / execute / drill-down |
| `q`                  | Quit                           |

### Actions

| Key     | Context            | Action                                          |
| ------- | ------------------ | ----------------------------------------------- |
| `Space` | Lists              | Toggle selection                                |
| `/`     | Packages           | Search / Filter                                 |
| `r`     | Global             | Refresh current screen/tab data                 |
| `k`     | Processes / Ghosts | Kill selected process                           |
| `a`     | Gateway / Tunnel   | Add route / create tunnel                       |
| `D`     | Gateway / Tunnel   | Delete selected route / ingress entry           |
| `f`     | Tunnel             | Toggle focus between Tunnels and Ingress panels |
| `s`     | Security           | Start new security scan                         |

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
│   ├── system/              # SystemInfo trait + sysinfo 0.30 impl
│   ├── packages/            # PackageManager trait + apt / dnf / pacman / brew
│   ├── processes/           # ProcessManager trait + sysinfo impl
│   ├── security/            # SecurityAuditor trait + SSH/ASLR checks
│   ├── firewall/            # FirewallManager trait + ufw / firewalld
│   ├── ssh/                 # SshKeyManager trait + authorized_keys / ssh-keygen
│   ├── docker/              # DockerManager trait + Docker Engine API
│   ├── wasm_cloud/          # wasmCloud management
│   ├── ghost/               # Ghost service detection logic
│   ├── gateway/             # GatewayManager trait + Caddy impl
│   └── tunnel/              # TunnelManager trait + cloudflared impl
├── db/
│   ├── mod.rs               # init_db (SQLite, auto-create ~/.postlab/data.db)
│   └── audit.rs             # Log actions for audit history
└── tui/
    ├── mod.rs               # Terminal init + main event loop
    ├── app.rs               # App state machine and background task management
    ├── events.rs            # Keyboard dispatch (global + screen-specific)
    └── screens/             # UI implementation for all 8 screens
```

---

## Support

### Package Managers

Detected automatically at startup:

- **Debian / Ubuntu**: `apt`
- **Fedora / RHEL**: `dnf` / `yum`
- **Arch**: `pacman`
- **macOS**: `brew`

### Security Hardening Audits

| Check                               | Severity | Action        |
| ----------------------------------- | -------- | ------------- |
| SSH root login enabled              | Critical | One-click fix |
| SSH password auth enabled           | High     | One-click fix |
| Firewall (ufw / firewalld) inactive | High     | One-click fix |
| ASLR not fully enabled              | Medium   | One-click fix |
| Auto-updates not configured         | Low      | One-click fix |

Every fix creates a `.bak.<timestamp>` copy of the config file first, e.g., `/etc/ssh/sshd_config.bak.20260303T142031`.

---

## Roadmap

- [x] **Docker** — Management of containers, images, and Compose.
- [x] **wasmCloud** — Host and component management.
- [x] **SSH Keys** — Interactive authorized_keys manager.
- [x] **Firewall** — UFW/firewalld rule management.
- [x] **Ghost Hunter** — Detect abandoned services.
- [ ] **Services** — Full systemd start / stop / restart / status manager.
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
