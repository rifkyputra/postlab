---
status: Ready-for-Implementation
feature: System Homelab tab
updated: 2026-08-13
---

# System Homelab tab

## Goal

Add a Linux-focused **System → Homelab** tab that reports and toggles four persistent server-stability settings without blocking the TUI:

1. **Keep it awake** — block manual and programmatic suspend/hibernate.
2. **Disable automatic sleep/hibernation** — configure systemd-logind to ignore idle and lid-triggered sleep.
3. **Wake-on-LAN** — enable or disable magic-packet wake on supported wired interfaces.
4. **Wi-Fi server stability** — disable or re-enable Wi-Fi power saving.

## Acceptance criteria

- `SystemTab` includes **Homelab**, navigable by keyboard and mouse.
- The tab shows each setting as Enabled, Disabled, Unavailable, or Error, with a short platform/interface detail.
- Up/Down selects a setting; Space or Enter opens a confirmation prompt; `r` refreshes.
- Every load and mutation runs in a Tokio background task; the TUI remains responsive.
- Successful and failed mutations are written to the existing SQLite audit log under action `homelab`.
- Mutations are idempotent and use argument-based process invocation, never interpolated shell commands.
- Postlab only removes state it owns. Managed config files are clearly named and writes are atomic.
- Linux/systemd is supported. Missing systemd, NetworkManager, `ethtool`, `iw`, or matching interfaces produce an honest Unavailable state rather than a panic.
- macOS and unknown hosts show all controls as unavailable; no Linux paths are changed.
- Unit tests exercise status parsing/config generation and command execution with temporary paths/fake executables.
- `feature_list.json` and the bundled postlab skill document the tab.
- `make check && make test` pass cleanly.

## Architecture

### Core

Create `cli/src/core/homelab/mod.rs` with:

- `HomelabFeature`: `KeepAwake`, `AutomaticSleep`, `WakeOnLan`, `WifiPowerSaving`.
- `HomelabFeatureStatus` and `HomelabStatus` data returned to the TUI.
- `HomelabManager`, owned by `Platform`, with async `status()` and `set(feature, enabled)` methods.
- Injectable filesystem root and executable paths for tests; production defaults target `/etc` and normal command lookup.

Semantics:

- **Keep awake on:** mask `sleep.target`, `suspend.target`, `hibernate.target`, and `hybrid-sleep.target`. Record only targets newly masked by Postlab in `/etc/postlab/homelab/keep-awake-targets`; off un-masks only those recorded targets.
- **Automatic sleep disabled on:** atomically write `/etc/systemd/logind.conf.d/90-postlab-homelab.conf` with `IdleAction=ignore` and all lid switch actions set to `ignore`; off removes only that managed drop-in. Ask logind to reload with HUP after a successful change.
- **Wake-on-LAN on/off:** detect physical wired interfaces from `/sys/class/net`, verify support with `ethtool`, apply runtime state (`wol g`/`wol d`), and persist via NetworkManager wired connection profiles when NetworkManager owns the device. If persistence is unavailable, report the setting as unavailable rather than claiming a durable toggle.
- **Wi-Fi stability on/off:** detect wireless interfaces from sysfs, apply runtime state using `iw ... power_save off/on`, and persist NetworkManager Wi-Fi profiles using powersave value `2` (disabled) or `3` (enabled). The feature's Enabled state means power saving is disabled.

For multi-interface settings, report Enabled only when all supported managed interfaces match; include affected interface names in detail. If no applicable interface/tool/backend exists, return Unavailable.

### TUI

Create `cli/src/tui/screens/homelab.rs`:

- Four-row selectable table/list with state colors and details.
- Intro text clarifying that Enabled means the named homelab optimization is active.
- Hints: `[↑/↓] select  [Space/Enter] toggle  [r] refresh`.

Extend `App` with `HomelabState`, task results for load/operation completion, background spawn methods, result handling, and error/loading cleanup. Add a confirmation action carrying the selected feature and desired state.

## Security and rollback

- Postlab already runs as root; do not add `sudo` subprocesses or weaken the root check.
- Never overwrite unrelated administrator config.
- Restrict generated files to fixed paths and fixed content; interface/profile values come from command output and are passed as individual arguments.
- Keep-awake rollback is ownership-aware so pre-existing masks remain masked.
- Managed writes use a temporary sibling plus rename; failures leave the previous state intact.
- Log operation, target feature, output/error, and success through `db::audit::log_action`.

## Verification

1. Unit tests for feature ordering/labels, logind config detection, keep-awake ownership parsing, interface filtering, NetworkManager output parsing, and generated command arguments via fake executables.
2. Fresh general/code-quality/security/testing review of the complete diff.
3. `make check && make test`.
4. HITL: user runs `sudo postlab`, opens System → Homelab, visually checks layout, and validates toggles on representative hardware. Do not self-certify the TUI visually.
