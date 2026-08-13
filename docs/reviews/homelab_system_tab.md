---
status: Ready-for-HITL
feature: System Homelab tab
reviewed: 2026-08-13
---

# Review evidence — System Homelab tab

## Scope

New `System → Homelab` tab with four Linux server-stability toggles: keep-awake
(systemd target masks), disable automatic sleep/hibernation (logind drop-in),
Wake-on-LAN (ethtool + NetworkManager), and Wi-Fi power saving disable (iw +
NetworkManager). Plan: `docs/plan/homelab_system_tab.md`.

## Review rounds

### Round 1 — cold parallel review (4 reviewers)

Ran `fy-general-reviewer`, `fy-security-reviewer`, `fy-testing-reviewer`,
`fy-quality-reviewer` cold against the full diff. Full reports were written under
`.featyard/reviews/2026-08-13/`.

Key findings (all subsequently addressed):

| # | Severity | Finding | Resolution |
|---|---|---|---|
| 1 | Important | Stale refresh race — mutation could run during load, older result displayed | `spawn_set_homelab` and Space/Enter keyguard now reject while `loading`; `mutating` guard retained |
| 2 | Important | `log_action` audit failures silently swallowed | Audit error carried in `HomelabOpDone { audit_error }`, surfaced in status bar |
| 3 | Important | Inactive NetworkManager profiles excluded via `DEVICE` (`--`) | Profile query now uses `UUID,TYPE,connection.interface-name`; test `inactive_associated_network_manager_profile_is_used` |
| 4 | Important | Partial network mutations without rollback | Full preflight/snapshot/apply/rollback transaction; `run_network_command` helper; rollback args recorded before each mutation; rollback-failure diagnostics aggregated |
| 5 | Important | Automatic-sleep status not effective-state aware | Disable now refuses when managed drop-in was modified (`modified; refusing to remove`); exact-bytes equality required for enable |
| 6 | Important | Pre-existing admin masks produce false success on disable | Disable re-checks effective state; reports `Error` when targets remain masked outside Postlab ownership |
| 7 | Important | Unrelated `TaskResult::Error` cleared `homelab.mutating` | Homelab flags removed from the global error arm; scoped op results manage state |
| 8 | Important | Crash could orphan a Postlab mask (mask before journal) | Ownership journal written *before* masking; rollback of journal on mask failure; stale-journal entries are parsed against fixed target list only |
| 9 | Important | Root subprocesses resolved via inherited `PATH` | `trusted_binary()` resolves systemctl/ethtool/nmcli/iw from `/usr/bin,/usr/sbin,/bin,/sbin` only |
| 10 | Minor | Wi-Fi status rejected `2 (disable)` enum output | Enum values parsed by numeric prefix (`64 (magic)`, `2 (disable)`); tests cover real nmcli formats |
| 11 | Important | Multi-interface/off paths untested | Added WOL-off, Wi-Fi-off, inactive-profile, mid-operation rollback, late-preflight (no-mutation), and rollback-failure tests |

### Round 2 — focused re-review

Dispatch of focused reviewers was aborted by the harness twice; the same areas were
verified manually against the final diff (see Verification below). No new material
findings surfaced in manual verification.

## Accepted tradeoffs (documented, not fixed)

- **Boolean `enabled` parameter on `set(feature, enabled)`** — reviewer Q1 suggested
  intent-revealing types. The API is internal to `HomelabManager` and callers pass
  literal `true`/`false` from a single keyguard; renaming would add churn without
  user-visible benefit. Accepted under the project's minimalism constraint.
- **`Option<PathBuf>` for tool paths (Q2)** — kept for test-injectable unavailable
  states; production always resolves via `trusted_binary()`.
- **Duplicated keep-awake state policy (Q11)** — read/write paths share the fixed
  `KEEP_AWAKE_TARGETS` list; the accepted `is-enabled` states are compared in two
  places but both are pinned to the same constant list. Accepted.

## Verification

- `make check` — PASS, zero warnings (cargo check + clippy `-D warnings`)
- `make test` — PASS, **140 tests** (up from 102 baseline; 38 homelab-specific)
- Unit coverage: all four features × enable/disable, idempotency, ownership rollback,
  admin-mask detection, logind modified-config refusal, inactive-profile persistence,
  nmcli enum parsing, WOL/Wi-Fi transactional rollback, unsupported-platform safety,
  interface filtering, argv-exact assertions via fake executables.
- TUI routing, SystemTab enum, click handling, confirmation dialog, task results, and
  audit integration compile-clean; **visual behavior requires `sudo postlab` (HITL)**.

## HITL gate

`tui/screens/*.rs` changed (new `homelab.rs` + `system.rs` routing). User must run
`sudo postlab`, open System → Homelab, and visually verify:
- Tab appears last in the System tab bar; keyboard (`L`/`H`), mouse, `r` refresh work.
- Each row shows Enabled/Disabled/Unavailable/Error with detail; Space/Enter prompts
  (y/N); confirmation applies and refreshes; status bar reports success/failure.
- On real hardware: keep-awake masks targets and restores them; logind drop-in
  disables idle/lid sleep; WOL persists via NetworkManager; Wi-Fi power save disables.
