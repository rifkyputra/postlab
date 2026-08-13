# v0.6.0 — 2026-08-13

Changes from `v0.5.0` to `v0.6.0` (4 commits).

## New: System → Homelab tab

Linux-only tab in the System screen with four persistent server-stability toggles:

- **Keep it awake** — masks `sleep/suspend/hibernate/hybrid-sleep.target` via systemd, with ownership-aware rollback (only Postlab-created masks are removed on disable).
- **Disable automatic sleep/hibernation** — writes a managed `systemd-logind.conf.d` drop-in (`IdleAction=ignore`, lid-switch ignores) and reloads logind via HUP.
- **Wake-on-LAN** — enables/disables magic-packet wake with `ethtool` (runtime) and persists per-interface via NetworkManager wired profiles, including inactive profiles.
- **Wi-Fi server stability** — disables Wi-Fi power saving with `iw` (runtime) and persists `802-11-wireless.powersave` via NetworkManager profiles.

Design highlights:

- Argument-based process invocation (no shell interpolation); tools resolved from trusted absolute paths (`/usr/bin`, `/usr/sbin`, `/bin`, `/sbin`).
- Network mutations are transactional: preflight snapshot → apply → best-effort rollback on failure with diagnostics.
- Every mutation is recorded in the SQLite audit log (`homelab` action); audit failures surface in the status bar.
- Honest `Unavailable` states on macOS/unknown hosts or when systemd/NetworkManager/ethtool/iw/interface support is missing.
- 38 unit tests for the module; full suite 140 tests; `make check` clean (zero clippy warnings).

## Other changes since v0.5.0

- **Node.js quick install** — NodeSource LTS setup script (apt) with `nsolid`.
- **New quick-install entries** — `bun` (curl installer, apt+dnf) and `claude-code` (npm, new Dev Tools category).
- **Pi Agent Auth tab** — interactive model/provider config manager: `[m]` edit default model, `[p]` cycle provider (persisted to `~/.pi/agent/settings.json`), `[l]` run `pi login <provider>` with streamed output.
- **Rust tooling** — `rustup` added to quick install.
- **Native git module** — `cli/src/core/git/` (repo, creds, error) replaces `deploy/git.rs`.
- Package manager adapter updates (apt/dnf) and web `dist` placeholder.
- Version bumped `0.3.0` → `0.6.0`.
