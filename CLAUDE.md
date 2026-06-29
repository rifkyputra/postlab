# postlab

Rust TUI bare-metal server manager. Runs as root. Stack: ratatui + crossterm, SQLite via sqlx, clap.

## Build & validate

| Command | Use |
|---|---|
| `make build` | Dev build |
| `make check` | Type-check + clippy (zero-warnings policy) |
| `make test` | Unit tests |
| `make run` | Launch TUI (requires root) |
| `make build-release` | Stripped, LTO release binary |
| `make build-linux` | Cross-compile to x86_64 Linux (requires cargo-zigbuild) |
| `make dev` | Watch + auto-restart (requires cargo-watch) |

**Before declaring any task done: run `make check && make test`. Both must pass cleanly.**

## Feature reference

**`feature_list.json`** (repo root) is the source of truth for all screens, tabs, and CLI commands. Read it before adding, renaming, or describing any feature — do not infer feature names or structure from the code alone.

**`PROGRESS.md`** (repo root) is the cross-session handoff file. Read it first in any new
session to discover in-flight work, blocked features, and recent merges. Update it at each
phase transition per `docs/agent_workflow.md` §6.

## Architecture

```
cli/src/
  main.rs              entrypoint: CLI parsing, DB init, root check
  tui/
    app.rs             App state + screen routing
    events.rs          Crossterm event loop
    screens/           One file per TUI screen (dashboard, security, docker, …)
  core/
    platform.rs        Platform detection (Ubuntu/Fedora/macOS)
    packages/          apt, dnf, brew, pacman adapters
    docker/            Docker CLI wrapper
    firewall/          ufw, firewalld, pf adapters
    gateway/           Caddy management
    deploy/            Git-based deploy runner
    security/          fail2ban, security checks
    system/            sysinfo wrapper
    ssh/ tunnel/ users/ portcheck/ processes/ …
  db/
    mod.rs             sqlx pool init
    audit.rs           Audit log
    deployments.rs     Deployment records
migrations/            Append-only SQL schema files
```

## Constraints

- **Root check** in `main.rs` — do not remove or weaken it.
- **`migrations/`** — SQL migrations are irreversible. Confirm with the user before creating or editing any `.sql` file.
- **`.github/workflows/`** — CI pipeline changes affect all build targets. Confirm before modifying.
- **`install.sh`** — runs with elevated privileges on user machines. Confirm before modifying.
- **`binaries/`** — pre-built release artifacts. Do not edit directly; use `make build-linux` or `make build-all`.
- **No new external services** (databases, queues, APIs) without explicit discussion.
- **`[release]` in commit message** — triggers CD artifact build. Only include this when the full matrix (linux/macOS, x86_64/ARM64) should build binaries. Use sparingly.

## Code style

- No comments unless the WHY is non-obvious (hidden constraint, workaround, subtle invariant).
- No docstrings or multi-line comment blocks.
- No abstractions beyond what the task requires — three similar lines beats a premature helper.
- No error handling for scenarios that can't happen; trust sqlx, tokio, and ratatui guarantees.
- Clippy warnings are errors. `make check` must pass with zero warnings before finishing.

## TUI changes

TUI screens can't be verified headlessly. If you change a screen and can't run `sudo postlab`, say so explicitly rather than claiming success.

## Agent finish checklist

Before closing any task:
1. `make check` passes (zero warnings)
2. `make test` passes
3. If a screen, tab, or CLI command was added or renamed: update `feature_list.json`
4. If TUI was changed: note that visual verification requires `sudo postlab`
5. If migrations touched: user confirmed
6. If CI/install.sh touched: user confirmed

## Multi-agent workflow

**For any non-trivial postlab feature work, follow `docs/agent_workflow.md`.**

Triggers: `do agentic workflow`, `spawn agents`, `run the full workflow`, `multi-agent`,
`fan out`, or `follow docs/agent_workflow.md` — all equivalent.

This document defines a multi-phase process (intake → architecture → recon → implement →
review → HITL → docs → release) using the agent runtime's `team` and `Agent` primitives.
Key rules:
- The parent agent orchestrates; subagents do the churn and hand back durable artifacts
  (`docs/plan/`, `docs/research/`, `docs/reviews/`).
- Reviews always run as a fresh, cold agent — never the author.
- Right-size per §5 of the doc: collapse phases for trivial work, fan out for
  adapter-wide or multi-screen features.

**Non-collapsible gates (always run):** QAS (`make check` + `make test`); migration
confirmation (user); security review for system-mutating diffs; HITL `sudo postlab` for
any `tui/screens/*.rs` change; CI/`install.sh` confirmation (user).
