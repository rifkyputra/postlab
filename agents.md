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
- `PROGRESS.md` — cross-session handoff. Read first in any new session to discover
  in-flight work, blocked features, and recent merges. Update at each phase transition
  per `docs/agent_workflow.md` §6.
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

## Multi-agent workflow

**For any non-trivial postlab feature work, follow `docs/agent_workflow.md`.**

Triggers: `do agentic workflow`, `spawn agents`, `run the full workflow`, `multi-agent`,
`fan out`, or `follow docs/agent_workflow.md` — all equivalent.

Key rules:
- The parent agent orchestrates; subagents do the churn and hand back durable artifacts
  (`docs/plan/`, `docs/research/`, `docs/reviews/`).
- Reviews always run as a fresh, cold agent — never the author.
- Right-size per §5 of the doc: collapse phases for trivial work, fan out for
  adapter-wide or multi-screen features.

**Non-collapsible gates (always run):** QAS (`make check` + `make test`); migration
confirmation (user); security review for system-mutating diffs; HITL `sudo postlab` for
any `tui/screens/*.rs` change; CI/`install.sh` confirmation (user).

### Single-phase shortcuts

| Want | Trigger |
|---|---|
| Research only | `team action='run', team='parallel-research', goal='...', skill='postlab'` |
| Implement only | `team action='run', team='implementation', goal='...', skill='postlab'` |
| Review only | `team action='run', team='review', goal='...', skill='postlab'` |
| Quick fix | `team action='run', team='fast-fix', goal='...', skill='postlab'` |
