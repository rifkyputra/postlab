# Changelog

All notable changes to this project will be documented in this file.

## [0.6.0] - 2026-08-13

- **System → Homelab tab** — persistent Linux server-stability toggles: keep awake (systemd sleep/suspend/hibernate target masks with ownership-aware rollback), disable automatic sleep/hibernation (systemd-logind drop-in + reload), Wake-on-LAN (ethtool runtime + NetworkManager profiles, transactional preflight/rollback, includes inactive profiles), and Wi-Fi server stability (disable Wi-Fi power saving via `iw` + NetworkManager). Mutations are audit-logged; tools resolved from trusted absolute paths; honest `Unavailable` states on unsupported hosts.
- Node.js quick install via NodeSource LTS setup (`nsolid`); `rustup`, `bun`, and `claude-code` (new Dev Tools category) added to quick install.
- Pi Agent Auth tab: interactive model/provider config manager (`[m]` edit model, `[p]` cycle provider, `[l]` streamed `pi login`).
- Native git module (`core/git/`) replacing `deploy/git.rs`; apt/dnf adapter updates.
- Added 38 unit tests for the Homelab module (140 total).

## [0.5.0] - 2026-07-09

- Fedora fixes: correct `dnf` upgradable list and firewalld rule handling; fix Caddy package repository path.
- Status tab first in the Agent screen; neovim added to quick install.
- Drop the global `s` hotkey conflict (System screen navigation now uses `1`–`9` / `Tab` only).
- Shell completions command (`postlab completions <shell>`) and TUI help overlay.

## [0.4.0] - 2026-07-03

- **musl static builds** for glibc-independent Linux binaries (installer defaults to the musl variant).
- Hardware tab (System screen): CPU temperatures, fan speeds, load-average history, and `systemd-analyze` boot-time breakdown; `[i]` installs lm-sensors.
- Installer now pulls per-platform tarballs from GitHub Releases.
- Static GitHub Pages landing page; vhs demo GIF.

## [0.3.0] - 2026-06-28

- User management screen — list, create, and delete users with password and sudoers handling.
- WasmCloud + NATS — service management screens, NATS backbone provisioning, and a component inspector.
- Managed Docker services — managed workloads, Podman support, and Firewalld/pf firewall managers.
- Pi Agent (formerly Zeroclaw) — chat, tools, tasks, sessions, skills library, auth, and logs tabs.
- Networking screen — Gateway (Caddy), Tunnel (Cloudflare), and Tailscale tabs.
- Projects and Automation/Agent screens.
- Swap management, Storage tab (System), and Updates tab (Packages).
- Non-interactive create-better-t-stack project scaffolding with stack configurator and addons.
- Installer now pulls per-platform tarballs from GitHub Releases (Linux x86_64/arm64, macOS arm64).
- Added 41 unit tests across docker, pi_agent, tunnel, and helper modules.

## [0.2.0] - 2026-03-03

- Bumped package version to `0.2.0`.
- Turn Postlab into a TUI app

<!--
Guidelines: keep entries brief. For future releases, add sections like:

## [0.1.1] - YYYY-MM-DD
- Fix: ...
--> 
