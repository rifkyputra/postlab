# dry-run mode — simulate mutations without executing them

## Overview

Dry-run mode lets a user preview any mutating postlab action without changing the system. When active, every mutation follows a **two-step protocol**: postlab first reports whether the underlying tool has a native dry-run, prints the exact command that would run, then executes the native dry-run if one exists — or executes nothing and says so when it doesn't.

It is off by default and never persisted: each invocation (CLI or TUI session) starts in normal mode unless explicitly enabled.

```
normal mode      run_mutation → execute for real
dry-run active   run_mutation → report capability
                              → print exact command
                              → native dry-run if available, else no-op
```

## Activation

Three ways to enable, all feeding one process-global flag:

- **CLI flag** — `--dry-run`, a global clap arg available on every subcommand
  (`postlab packages install nginx --dry-run`).
- **Env var** — `POSTLAB_DRY_RUN=1`, for scripting and CI. Applies to CLI and TUI.
- **TUI toggle** — **Ctrl-D** flips dry-run for the whole session, across all
  screens, with a persistent `DRY-RUN` badge in the status bar.

State lives in a process-global `AtomicBool` (`core::dryrun::is_active()` /
`set_active()`), seeded in `main.rs` from `--dry-run` OR `POSTLAB_DRY_RUN`, and
flippable at runtime by the TUI toggle.

## Why the gate lives in a shared helper, not in `run_cmd`

A central classifier inside `run_cmd` / `run_cmd_streaming`
(`cli/src/core/packages/mod.rs:124`) was rejected for two reasons:

1. Those helpers are also the **read path**. Gating them needs a fragile "is this
   command mutating?" classifier, and ~140 `Command::new` sites bypass them anyway.
2. The two-step semantics need **per-command knowledge** — apt has `--dry-run`,
   `systemctl enable` has `--dry-run`, but `systemctl start` and `swapon` have
   none. That knowledge belongs at the call site, not in a generic wrapper.

So the design is: **one helper that encodes the protocol, each call site supplies
the native variant.** Read paths keep using `run_cmd` untouched.

## The mutation helper

New module `cli/src/core/dryrun.rs`:

```rust
pub struct Mutation<'a> {
    program: &'a str,
    args: &'a [&'a str],
    native_dry: Option<&'a [&'a str]>,  // e.g. Some(&["install","--dry-run","nginx"])
    label: &'a str,                     // human action, for output + audit
}

pub async fn run_mutation(m: Mutation<'_>) -> Result<String>;
pub async fn run_mutation_streaming(
    m: Mutation<'_>,
    tx: tokio::sync::mpsc::UnboundedSender<String>,
) -> Result<String>;
```

- **Dry-run inactive** → behaves exactly like `run_cmd` today (executes
  `program args`).
- **Dry-run active** → emits the two-step report, runs `native_dry` if `Some`,
  returns its output; if `None`, executes nothing and returns a
  "not executed — no native dry-run" notice.

The streaming sibling exists for the package install/remove TUI paths that already
stream output line-by-line.

### Multi-step operations

Operations with no single native dry-run (swap create/resize, git deploy) print
the **full command sequence** in order — each underlying step is modeled as data
and listed, none executed. Example for `swap create`:

```
[dry-run] swap create /swapfile (2048 MiB) — no native dry-run, not executed
would run:
  fallocate -l 2048M /swapfile
  chmod 600 /swapfile
  mkswap /swapfile
  swapon /swapfile
  (append to /etc/fstab) /swapfile none swap sw 0 0
```

## Per-domain wiring

Scope order: **Packages → Services & system → Deploy & Docker.**
Firewall & SSH are deliberately deferred to a later pass.

| Domain     | Method                    | Native dry-run                         |
|------------|---------------------------|----------------------------------------|
| apt        | install / remove / upgrade | `apt-get … --dry-run` ✓               |
| apt        | cache-clean               | none → print only                      |
| dnf        | install / remove / upgrade | `dnf … --assumeno` ✓                  |
| systemd    | enable / disable          | `systemctl … --dry-run` ✓             |
| systemd    | start / stop / restart    | none → print only                      |
| swap       | create / resize / delete  | none (multi-step) → print sequence     |
| processes  | kill                      | none → print only                      |
| deploy     | git runner steps          | none → print command list              |
| docker     | compose up / down / pull  | `docker compose … --dry-run` ✓ (v2.20+) |
| docker     | container start/stop/rm    | none → print only                      |

## Audit log

Every simulated action is logged via the existing `log_action`
(`cli/src/db/audit.rs`) with a `[dry-run]` marker in the `output` field, so history
shows intent without implying a real change. **No schema change** — the
`migrations/` constraint is not triggered.

## TUI changes

- **Ctrl-D handler** at the top of `handle_key` (`cli/src/tui/events.rs:12`),
  before any per-screen routing. The dispatcher currently inspects only `KeyCode`
  and never reads modifiers, so Ctrl-D is free (bare `d` is bound per-screen in 12
  places, but no Ctrl combo exists today).
- **Status-bar badge** — a persistent `DRY-RUN` indicator while active, so it is
  always obvious the session is simulated.

> The Ctrl-D toggle and badge **cannot be verified headlessly**. Visual
> confirmation requires `sudo postlab`.

## Build order

1. `core/dryrun.rs` — global state + `run_mutation` / `run_mutation_streaming`.
2. `main.rs` — global `--dry-run` arg + `POSTLAB_DRY_RUN` env seed.
3. Wire Packages adapters (apt/dnf), then Services/system, then Deploy/Docker.
4. Audit `[dry-run]` flagging.
5. TUI — Ctrl-D handler in `handle_key` + status-bar badge.
6. `feature_list.json` — document `--dry-run`, the env var, and the TUI toggle.
7. `make check && make test`.
