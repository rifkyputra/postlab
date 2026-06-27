---
name: postlab
description: Postlab bare-metal server manager. Use when the user wants to manage services, install packages, configure firewalls, manage Docker containers, set up reverse proxies or tunnels, audit security, manage users, create swap, browse projects, or interact with the pi agent — all through Postlab's TUI or CLI. Covers all 9 screens and one-shot CLI commands.
---

# Postlab Server Manager

Postlab is an interactive bare-metal server manager that runs directly on the machine it manages. Single binary, low memory, cross-platform (Linux + macOS). Must run as root.

## Execution Model

Pi executes tool calls sequentially, even when you emit multiple calls in one turn. Batch independent calls in a single turn to save round-trips:

| Pattern | Use for |
|---------|---------|
| Multiple `bash` calls in one turn | Independent diagnostics (df & free & uptime) |
| `postlab info` or `postlab list` via bash | One-shot system queries |

**Important**: When the user asks you to perform a server management task, guide them to the right Postlab screen and actions. Postlab is their tool — you're showing them how to use it, not replacing it with raw shell commands. However, Postlab can't be run headlessly in agent mode, so when the user asks you to *do* something on the server (not just *show* how), use direct shell commands (`systemctl`, `apt`, `docker`, etc.) via the ubuntu-sysadmin skill instead.

## CLI Commands

```bash
sudo postlab                  # Launch TUI (default)
sudo postlab info             # OS, kernel, CPU, memory, uptime
sudo postlab list             # All installed packages
sudo postlab system info      # System diagnostics
sudo postlab system cpu       # CPU details
sudo postlab system mem       # Memory details
sudo postlab system disks     # Disk usage
sudo postlab system net       # Network interfaces
sudo postlab system swap status          # Swap status
sudo postlab system swap create <size>   # Create swap file (e.g., 2G)
sudo postlab system swap delete          # Delete swap file
sudo postlab system swap enable|disable  # Toggle swap
sudo postlab system swap resize <size>   # Resize swap file
sudo postlab packages list              # Installed packages
sudo postlab packages search <query>    # Search package index
sudo postlab packages install <pkg>     # Install a package
sudo postlab packages remove <pkg>      # Remove a package
sudo postlab packages upgrade           # Upgrade all packages
sudo postlab packages cache-clean       # Clean package cache
sudo postlab packages list-upgradable   # List available package updates
sudo postlab storage list               # Filesystems + physical disks (SMART health)
sudo postlab processes list             # List processes (--sort cpu|mem|pid|name)
sudo postlab processes kill <pid>       # Kill process (--force for SIGKILL)
sudo postlab services list              # List systemd/launchd services
sudo postlab services start|stop|restart <unit>
sudo postlab services enable|disable <unit>
sudo postlab --database <path>          # Custom DB path (default: ~/.postlab/data.db)
```

## Screen Reference

Postlab has 9 screens, accessed by pressing `1`–`9` or clicking the nav bar.

### 1. Dashboard — `key: 1`

| Tab | Use for |
|-----|---------|
| **Overview** | Live hostname, OS, uptime, CPU cores, memory/disk/swap gauges |
| **Processes** | Live process table sorted by CPU or memory; `k` to kill selected |
| **Resources** | Per-core CPU sparklines, memory and swap history over 60 seconds |

### 2. Packages — `key: 2`

| Tab | Use for |
|-----|---------|
| **Installed** | Browse all installed packages with versions |
| **Search** | Search the distro package index; `/` to filter |
| **Quick Install** | Curated list of common server packages; one-keypress install |
| **Queue** | Batch queue multiple packages, then install all together |
| **Updates** | View available upgrades with version deltas, security flagging, and one-key upgrade-all |

Press `Space` to toggle selection, `Enter` to install/remove. In Updates tab: `Space` select, `u` upgrade selected, `U` upgrade all.

### 3. Security — `key: 3`

| Tab | Use for |
|-----|---------|
| **Findings** | Auto-scans on first visit: SSH root login, password auth, firewall status, ASLR, auto-updates. Each finding has a severity and one-click fix. Re-scan after applying fixes. |
| **Firewall** | View/manage UFW / firewalld / pf rules. Add/delete rules by port and protocol. |
| **Ports** | Check open listening ports and identify services bound to them |
| **SSH** | Manage authorized_keys entries; view public key fingerprints |
| **Fail2Ban** | View Fail2Ban status, jails, and banned IPs; ban/unban manually |

Every fix creates a timestamped `.bak` backup first (e.g., `/etc/ssh/sshd_config.bak.20260303T142031`).

Security checks:
| Check | Severity | One-click fix |
|-------|----------|---------------|
| SSH root login enabled | Critical | Yes |
| SSH password auth enabled | High | Yes |
| Firewall inactive | High | Yes |
| ASLR not fully enabled | Medium | Yes |
| Auto-updates not configured | Low | Yes |

### 4. Networking — `key: 4`

| Tab | Use for |
|-----|---------|
| **Gateway** | Install Caddy reverse proxy; add/delete routes (domain → port) with automatic TLS. Press `a` to add, `D` to delete. |
| **Tunnel** | Install cloudflared; create Cloudflare tunnels with route management and ingress configuration. Press `a` to add, `D` to delete, `f` to toggle Tunnels/Ingress panels. |
| **Tailscale** | Install Tailscale; view VPN status and connected peers; bring up/down the VPN connection |

### 5. Docker — `key: 5`

| Tab | Use for |
|-----|---------|
| **Containers** | List running and stopped containers; start, stop, remove |
| **Images** | Browse local images with size and tag info |
| **Compose** | Manage Docker Compose stacks: up, down, pull |
| **Workloads** | Host-managed single-service workloads (systemd integration). Linux+systemd only. Press `a` to create. |
| **Managed** | One-click dev services: PostgreSQL, Redis, MySQL, MongoDB, Elasticsearch, MinIO, MailHog, RabbitMQ |

Detects container engine automatically: prefers `docker`, falls back to `podman`.

### 6. wasmCloud — `key: 6`

| Tab | Use for |
|-----|---------|
| **Hosts** | View connected wasmCloud hosts and their status |
| **Components** | Browse deployed Wasm components across hosts |
| **Apps** | Manage wasmCloud application manifests |
| **Inspector** | Inspect component details, links, and capability providers |

### 7. Agent — `key: 7`

| Tab | Use for |
|-----|---------|
| **Chat** | Interactive chat with pi agent over JSONL RPC; streams text deltas in real time |
| **Tools** | Log of tool executions (read, bash, edit, write) during the active session |
| **Tasks** | Create background agent jobs with configurable intervals (30m–24h); toggle, run on demand, persisted in SQLite |
| **Status** | Pi agent install status, version, default model, and quick actions |
| **Sessions** | Browse saved pi session JSONL files in `~/.pi/sessions/` |
| **Config** | View `~/.pi/agent/settings.json` with secret masking and search |
| **Auth** | View configured provider credentials from `~/.pi/auth.json` |
| **Skills** | Browse and remove installed pi skills from `~/.pi/agent/npm/node_modules/` |
| **Library** | Browse curated pi skill library; install skills with a single keypress |
| **Logs** | Tail the most recent pi session log |

The agent process runs under the original user's UID/GID (via `$SUDO_UID`/`$SUDO_GID`), never as root.

### 8. System — `key: 8`

| Tab | Use for |
|-----|---------|
| **Ghosts** | Find orphaned/ghost processes reparented to PID 1; `k` to kill |
| **Janitor** | Package-cache cleanup to free disk space |
| **Services** | Browse and manage systemd/launchd services: `s` start, `k` stop, `r` restart, `e` enable, `d` disable |
| **Users** | Unix user management: list, add, remove, change passwords, manage sudoers |
| **Swap** | View swap usage; create, resize, enable, disable swap files |
| **Storage** | Filesystems table (device, mount, type, size, used, avail, %), physical disk SMART health (model, status, temperature, power-on hours), mount/unmount volumes, inspect /etc/fstab |

### 9. Projects — `key: 9`

| Tab | Use for |
|-----|---------|
| **Projects** | Browse local projects by last modified time |
| **New** | Scaffold a new app via `create-better-t-stack` (streams npx output) |
| **Clone** | Clone a GitHub repo by `user/repo` shorthand or full URL |
| **Settings** | Configure the projects directory (persisted in SQLite); shows active `$EDITOR` |

## Keybindings

### Navigation
| Key | Action |
|-----|--------|
| `1`–`9` | Switch screens |
| `a` | Jump to Agent (global) |
| `s` | Jump to System (global) |
| `Tab` / `Shift+Tab` | Next / previous screen |
| `H` / `L` or `←` `→` | Switch tabs within a screen |
| `↑` `↓` | Navigate lists or tables |
| `Enter` | Confirm / execute / drill-down |
| `q` | Quit |

### Actions
| Key | Context | Action |
|-----|---------|--------|
| `Space` | Lists | Toggle selection |
| `/` | Packages, Services, Config | Search / filter |
| `r` / `R` | Global | Refresh current screen/tab |
| `k` | Processes / Ghosts | Kill selected process |
| `a` | Gateway / Tunnel / Workloads | Add route / tunnel / workload |
| `D` | Gateway / Tunnel | Delete selected route / ingress entry |
| `f` | Tunnel | Toggle focus between Tunnels and Ingress panels |
| `s` / `k` / `r` | System → Services | Start / stop / restart selected unit |
| `e` / `d` | System → Services | Enable / disable selected unit |

Mouse clicks work on nav bar tabs, sub-tabs, and list items.

## Common Workflows

### "I need to install packages"
→ Screen **2 (Packages)**. Search for the package, press `Space` to queue it, repeat, then run the queue. Or use Quick Install for common packages.

### "Is my server secure?"
→ Screen **3 (Security)** → **Findings** tab. Auto-scans on first visit. Apply fixes with one keypress. Then check **Firewall** and **Ports**.

### "I want to expose a service with HTTPS"
→ Screen **4 (Networking)** → **Gateway** tab. Install Caddy, then add a route: domain → localhost:port. TLS is automatic.

### "I want a Cloudflare tunnel"
→ Screen **4 (Networking)** → **Tunnel** tab. Install cloudflared, create a tunnel, configure ingress rules.

### "I need a PostgreSQL container"
→ Screen **5 (Docker)** → **Managed** tab. One-click deploy PostgreSQL, Redis, MySQL, MongoDB, Elasticsearch, MinIO, MailHog, or RabbitMQ.

### "A service won't start"
→ Screen **8 (System)** → **Services** tab. Find the unit, check its status. Use `s`/`k`/`r` to start/stop/restart. Check **Ghosts** tab for orphaned processes.

### "I'm running out of disk space"
→ Screen **8 (System)** → **Storage** tab to browse filesystem usage. → **Janitor** tab to clean package caches. → **Swap** tab to check swap usage.

### "I need to add a user"
→ Screen **8 (System)** → **Users** tab. Add users, set passwords, manage sudoers.

### "I want to check Tailscale VPN status"
→ Screen **4 (Networking)** → **Tailscale** tab. View connected peers, bring VPN up/down.

### "I want to clone a GitHub repo"
→ Screen **9 (Projects)** → **Clone** tab. Paste `user/repo` or full URL. Or use **New** to scaffold with `create-better-t-stack`.

### "I need a quick system summary"
→ CLI: `sudo postlab info` (no TUI needed).

## Platform Support

Postlab auto-detects the host environment at startup:

| Subsystem | Linux | macOS |
|-----------|-------|-------|
| Package manager | apt, dnf, pacman | brew |
| Firewall | ufw, firewalld | pf |
| Container engine | docker, podman | docker, podman |
| Service manager | systemd | launchd |
| Workload backend | Quadlet (podman) or Compose+systemd (docker) | unavailable |

## Non-blocking Operations

All operations (package installs, docker commands, security scans) run in background tasks. The TUI stays responsive. Long-running operations stream output in real time in the relevant screen.

## Config Backups

Every destructive config change creates a timestamped `.bak` file:
- `/etc/ssh/sshd_config.bak.<timestamp>`
- `/etc/ufw/*.bak.<timestamp>`
- etc.

## Database

Postlab persists state in SQLite at `~/.postlab/data.db` (configurable with `--database`). Stores audit log, deployment records, agent task schedules, and projects config.
