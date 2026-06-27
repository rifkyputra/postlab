# Changelog

All notable changes to this project will be documented in this file.

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
