# Postlab

Interactive bare metal server manager — runs directly on the machine it manages.

Single binary. Low memory. Cross-platform (Linux + macOS).

<div style="width:600px; height:300px; overflow:hidden;">
  <img
    src="https://github.com/user-attachments/assets/ee968adf-a1b8-4b75-8457-12337c776395"
    style="width:100%; height:100%; object-fit:cover; object-position:center;"
    alt="Postlab Logo High Resolution"
  />
</div>
---

## Features

| Screen | What it does |
|--------|-------------|
| **Dashboard** | Hostname, OS, uptime, live CPU cores, memory, disk gauges |
| **Packages** | Install / remove / upgrade packages; curated quick-install list; operation queue |
| **Processes** | Sortable process table; kill with confirmation |
| **Security** | SSH, firewall, ASLR, update audits; one-key fix with `.bak` backup |
| **Resources** | CPU sparklines per-core, memory %, network RX/TX history |
| **Gateway** | Caddy install, route management (domain → port), TLS auto |
| **Tunnel** | Cloudflare tunnel create, route, install as service |

All operations are non-blocking — the TUI stays responsive while packages install.
Every destructive change to config files creates a timestamped `.bak` backup first.

---

## Quick Start

```bash
# Build
cargo build -p postlab --release

# Run TUI (default)
./target/release/postlab

# One-shot commands
./target/release/postlab info    # print system summary
./target/release/postlab list    # print installed packages
```

Or use `make`:

```bash
make build          # dev build
make run            # run TUI
make release        # optimised release build (~8–15 MB)
```

---

## Keybindings

| Key | Action |
|-----|--------|
| `1`–`7` | Switch screens |
| `Tab` / `Shift+Tab` | Next / previous screen |
| `↑` `↓` | Navigate list |
| `Space` | Toggle selection |
| `Enter` | Confirm / execute |
| `/` | Search (packages screen) |
| `r` | Refresh |
| `k` | Kill process (processes screen) |
| `a` | Add route / create tunnel |
| `D` | Delete selected route / tunnel |
| `q` | Quit |

---

## Architecture

```
cli/src/
├── main.rs                  # clap entry: info | list | tui (default)
├── core/
│   ├── platform.rs          # Platform { system, packages, processes,
│   │                        #            security, gateway, tunnel }
│   │                        # detect() — auto-selects right impls at runtime
│   ├── models.rs            # shared data types
│   ├── system/              # SystemInfo trait + sysinfo 0.30 impl
│   ├── packages/            # PackageManager trait + apt / dnf / pacman / brew
│   ├── processes/           # ProcessManager trait + sysinfo impl
│   ├── security/            # SecurityAuditor trait + SSH/firewall/sysctl checks
│   ├── gateway/             # GatewayManager trait + Caddy impl
│   └── tunnel/              # TunnelManager trait + cloudflared impl
├── db/
│   ├── mod.rs               # init_db (SQLite, auto-create)
│   └── audit.rs             # log_action(), recent() — audit log
└── tui/
    ├── mod.rs               # run() — terminal init + event loop
    ├── app.rs               # App state machine, background task channel
    ├── events.rs            # keyboard dispatch
    └── screens/             # dashboard, packages, processes, security,
                             # resources, gateway, tunnel
```

The `core/` layer has no TUI dependency — it can be imported by a future axum API with no code changes.

---

## Package Manager Support

Detected automatically at startup:

| OS | Package manager |
|----|----------------|
| Debian / Ubuntu | `apt` |
| Fedora / RHEL | `dnf` / `yum` |
| Arch | `pacman` |
| macOS | `brew` |

Curated quick-install categories: **Web Servers**, **Databases**, **System Tools**, **Runtimes**, **Security**.

---

## Security Hardening

The security screen runs these checks:

| Check | Severity |
|-------|----------|
| SSH root login enabled | Critical |

---

## License

This project is licensed under the Apache License, Version 2.0.
See the [LICENSE](LICENSE) file for details.

| SSH password authentication enabled | High |
| Firewall (ufw / firewalld) inactive | High |
| ASLR not fully enabled | Medium |
| Automatic security updates not configured | Low |

Applying a fix always creates a `.bak.<timestamp>` copy of the config file first, e.g.:
```
/etc/ssh/sshd_config.bak.20260303T142031
```

---

## Development

### Requirements

- Rust 1.75+
- SQLite (bundled via `sqlx` / `libsqlite3-sys`)

### Build & run

```bash
# From workspace root
cargo build -p postlab                 # dev build
cargo run -p postlab                   # run TUI
cargo run -p postlab -- info           # OS summary
cargo run -p postlab -- list           # installed packages
cargo build -p postlab --release       # release build (~8–15 MB)

# From cli/ directory
cargo build
cargo run
cargo run -- info
```

### Release binary size

The `[profile.release]` in `cli/Cargo.toml` is set to:

```toml
strip = true
lto = true
opt-level = "z"
codegen-units = 1
```

Expected output: **8–15 MB** (ratatui + sysinfo + sqlx, no server framework).

---

## Roadmap

- [ ] Docker — install, container list, Compose stacks
- [ ] Services — systemd start / stop / restart / status
- [ ] Firewall — UFW / firewalld rule management UI
- [ ] SSH key management
- [ ] Web API (axum) — expose `core::Platform` over HTTP
