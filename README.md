# Postlab

Bare-metal VPS management for developers — minimal DevOps knowledge required.

Manage servers, install apps, harden security, and monitor status via a CLI, REST API, or web UI. Designed to work equally well for human operators and agentic AI systems.

---

## Architecture

```
┌─────────────────────────────────────────────────────┐
│                    Postlab                           │
│  ┌──────────┐   ┌──────────────┐   ┌────────────┐  │
│  │   CLI    │──▶│   Backend    │◀──│  Web UI    │  │
│  │  (Rust)  │   │  (Axum API)  │   │ (Svelte)   │  │
│  └──────────┘   └──────┬───────┘   └────────────┘  │
│                        │                            │
│                 ┌──────┴───────┐                    │
│                 │  SQLite DB   │                    │
│                 └──────┬───────┘                    │
│                        │                            │
│              ┌─────────┴──────────┐                 │
│              │   SSH Agent        │                 │
│              │   (russh async)    │                 │
│              │  ┌───────┬───────┐ │                 │
│              │  │Ubuntu │Fedora │ │                 │
│              │  │(apt)  │(dnf)  │ │                 │
│              │  └───────┴───────┘ │                 │
│              └────────────────────┘                 │
└─────────────────────────────────────────────────────┘
```

## Project Structure

```
postlab/
├── Cargo.toml                  # Workspace root
├── crates/
│   ├── postlab-core/           # Shared: DB, SSH, task engine, OS detection, modules
│   ├── postlab-cli/            # `postlab` binary — interactive CLI
│   └── postlab-server/         # `postlab-server` binary — Axum REST API
├── web/                        # SvelteKit + TypeScript frontend
├── migrations/                 # SQLite schema (001_initial.sql)
├── harden-security/            # Shell scripts for Linux hardening
└── Makefile
```

## Quick Start

### 1. Start the API server

```bash
cargo run -p postlab-server
# Listens on http://0.0.0.0:3000
# Creates postlab.db automatically on first run
# Override DB path: DATABASE_URL=sqlite:///path/to/db.sqlite cargo run -p postlab-server
```

### 2. Use the CLI

```bash
cargo run -p postlab-cli                    # Interactive wizard
cargo run -p postlab-cli -- server add      # Add a server
cargo run -p postlab-cli -- server list     # List servers
cargo run -p postlab-cli -- server info <id>   # Details + live status
cargo run -p postlab-cli -- server remove <id>
cargo run -p postlab-cli -- task list       # Recent tasks
cargo run -p postlab-cli -- task show <id>  # Task output
```

The CLI connects to `http://localhost:3000` by default. Override with:

```bash
POSTLAB_URL=http://my-server:3000 postlab server list
```

### 3. Web UI

```bash
cd web && npm install && npm run dev
# Opens on http://localhost:5173 — proxies /api to :3000
```

---

## REST API

All routes are under `/api/`. Responses are JSON.

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/servers` | List servers |
| POST | `/api/servers` | Add a server |
| GET | `/api/servers/:id` | Server details |
| DELETE | `/api/servers/:id` | Remove server |
| GET | `/api/servers/:id/status` | Live metrics via SSH |
| POST | `/api/servers/:id/install` | Install app → creates task |
| POST | `/api/servers/:id/upgrade` | Upgrade OS → creates task |
| POST | `/api/servers/:id/harden` | Harden security → creates task |
| GET | `/api/tasks` | List tasks (`?server_id=` filter) |
| POST | `/api/tasks` | Submit task |
| GET | `/api/tasks/:id` | Task details + output |
| GET | `/api/audit` | Audit log |
| GET | `/api/config` | Config entries |
| PUT | `/api/config/:key` | Set config value |

### Example: add a server

```bash
curl -X POST http://localhost:3000/api/servers \
  -H 'Content-Type: application/json' \
  -d '{"name":"web-01","host":"1.2.3.4","user":"ubuntu","ssh_key_path":"~/.ssh/id_ed25519"}'
```

### Example: get live status

```bash
curl http://localhost:3000/api/servers/<id>/status
# {"uptime":"up 3 days","load":"0.12 0.08 0.07","memory":"...","disk":"..."}
```

---

## OS Detection

On first SSH connection, Postlab reads `/etc/os-release` on the target server and stores the result. Supported:

| Distro | Package manager |
|--------|----------------|
| Ubuntu / Debian | `apt` |
| Fedora / RHEL / Rocky / AlmaLinux | `dnf` |

---

## Security Hardening

`harden-security/` contains modular, idempotent hardening scripts (numbered 01–11). The security module executes them remotely via SSH.

Run all modules:

```bash
# remotely (via postlab)
postlab harden --server <id>

# directly on a server
sudo ./harden-security/run-enable-all.sh
```

Individual modules:

| # | Module | What it does |
|---|--------|-------------|
| 01 | update | System updates |
| 02 | password-policy | PAM password complexity |
| 03 | ssh-disable-root | `PermitRootLogin no` |
| 04 | unattended | Automatic security upgrades |
| 05 | firewall | UFW / firewalld + allow SSH |
| 07 | cleanup | Remove unused packages |
| 08 | shadow-perms | `/etc/shadow` permissions |
| 09 | disable-services | Stop avahi-daemon, cups |
| 10 | sysctl | Kernel network hardening |
| 11 | cron-restrict | Restrict cron to root |

---

## End-to-End Testing with Docker

The `docker/` directory provides two SSH-accessible containers — Ubuntu 22.04 and Fedora 39 — for testing the full CLI → API → SSH flow locally without a real VPS.

### Prerequisites

- Docker Desktop (or Docker Engine + Compose plugin)
- `postlab-server` built (`cargo build -p postlab-server`)

### Setup (one-time)

```bash
# 1. Generate an SSH keypair for the test containers
make docker-keygen

# 2. Start Ubuntu (:2222) and Fedora (:2223) containers
make docker-up
```

Output:
```
Containers ready:
  Ubuntu  → localhost:2222  (user: postlab, key: docker/test_key)
  Fedora  → localhost:2223  (user: postlab, key: docker/test_key)
```

### Run the full stack

```bash
# Terminal 1 — API server
make server

# Terminal 2 — CLI operations
# Add the Ubuntu container
cargo run -p postlab-cli -- server add \
  --name ubuntu-test \
  --host localhost \
  --port 2222 \
  --user postlab \
  --key $(pwd)/docker/test_key

# Add the Fedora container
cargo run -p postlab-cli -- server add \
  --name fedora-test \
  --host localhost \
  --port 2223 \
  --user postlab \
  --key $(pwd)/docker/test_key

# List registered servers
cargo run -p postlab-cli -- server list

# Fetch live status (SSH connect + OS detection + metrics)
cargo run -p postlab-cli -- server info <id>

# Or using the API directly
curl http://localhost:3000/api/servers
curl http://localhost:3000/api/servers/<id>/status
```

### Open a shell into a container

```bash
make docker-ssh-ubuntu   # SSH into Ubuntu container
make docker-ssh-fedora   # SSH into Fedora container
```

### Tear down

```bash
make docker-down   # Stop and remove containers
```

The test keypair (`docker/test_key`, `docker/test_key.pub`) is gitignored and must be regenerated per machine.

---

## Development

### Requirements

- Rust 1.85+ (edition 2024)
- Node.js 20+
- SQLite (bundled via `libsqlite3-sys`)

### Build everything

```bash
cargo build --workspace          # All three Rust crates
cd web && npm install && npm run build   # SvelteKit frontend
```

### Run tests

```bash
cargo test --workspace           # Rust unit tests
cd web && npm test               # Vitest
```

### Crate overview

| Crate | Type | Purpose |
|-------|------|---------|
| `postlab-core` | lib | DB, SSH (russh), task engine, OS detection, module stubs |
| `postlab-cli` | bin | Interactive CLI, all commands call the API via reqwest |
| `postlab-server` | bin | Axum REST API, SQLite via sqlx, task queue |

---

## Roadmap

Feature modules are stubs and will be implemented incrementally:

- [ ] `packages` — `apt`/`dnf` install/upgrade/remove/list
- [ ] `docker` — install Docker, manage containers and Compose stacks
- [ ] `services` — systemd start/stop/restart/status
- [ ] `firewall` — UFW / firewalld rule management
- [ ] Password auth support for SSH
- [ ] SSH known-hosts verification
- [ ] Web UI actions (install, upgrade, harden buttons)
- [ ] Task log streaming (SSE / WebSocket)
- [ ] Multi-server bulk operations
