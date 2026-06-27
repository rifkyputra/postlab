# postlab — agent instructions

Rust TUI bare-metal server manager. Stack: ratatui + crossterm, SQLite via sqlx, clap. Runs as root.

## Build & validate

| Command | Use |
|---|---|
| `make check` | Type-check + clippy (zero-warnings policy) |
| `make test` | Unit tests |
| `make build` | Dev build |
| `make run` | Launch TUI (requires root) |

**Before declaring any task done: `make check && make test` must pass cleanly.**

## Release trigger

Include `[release]` in a commit message to trigger the CD artifact build (full matrix: linux/macOS × x86_64/ARM64). Use sparingly — only when binaries need to ship.

## Key constraints

- Root check in `main.rs` — never remove or weaken.
- `migrations/` — irreversible. Confirm before editing `.sql` files.
- `.github/workflows/` — confirm before modifying.
- `install.sh` — confirm before modifying.
- No new external services without discussion.
- Clippy warnings are errors. Zero-warning policy.

## Source of truth

- `feature_list.json` — screens, tabs, CLI commands. Update when adding or renaming a feature.
- TUI changes cannot be verified headlessly. If you change a screen and can't run `sudo postlab`, say so explicitly.

## Architecture

```
cli/src/
  main.rs              CLI parsing, DB init, root check
  tui/
    app.rs             App state + screen routing
    events.rs          Crossterm event loop
    screens/           One file per TUI screen
  core/
    platform.rs        Ubuntu/Fedora/macOS detection
    packages/          apt, dnf, brew, pacman adapters
    docker/            Docker CLI wrapper
    firewall/          ufw, firewalld, pf adapters
    gateway/           Caddy management
    tunnel/            Cloudflare Tunnel
    tailscale/         Tailscale VPN
    deploy/            Git-based deploy runner
    security/          fail2ban, security checks
    system/            sysinfo + swap management
    storage/           lsblk, smartctl, mount/umount
    projects/          Local project browser + scaffolder
    pi_agent/          Pi Agent CLI integration
  db/
    mod.rs             SQLite pool init
    audit.rs           Audit log
    deployments.rs     Deployment records
    agent_tasks.rs     Scheduled agent task CRUD
migrations/            Append-only SQL schema files
```
