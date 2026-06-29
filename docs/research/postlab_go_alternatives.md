# postlab go — Alternative Approaches Comparison

Source: `docs/plan/postlab_go.md`

## 1. Web server process model

| Approach | Pros | Cons | Plan decision |
|---|---|---|---|
| **Inline in the same binary** (chosen) | Same tokio runtime, same SQLite pool, no IPC, single release artifact, faster startup. | A panic/crash in the web layer brings down the TUI entrypoint too; cannot run TUI and web simultaneously. | Chosen for v1. |
| **Child process** (`std::process::Command`) | Crash isolation; TUI and web could theoretically run together; easier privilege separation. | IPC complexity, separate logs, second binary or hidden subcommand, harder shared state, more packaging work. | Rejected for v1; may revisit for daemon mode. |
| **Separate daemon + TUI as client** | Clean architecture; web always available; both interfaces usable at once. | Largest change; requires systemd/PID file/journald integration; conflicts with current single-binary goal. | Listed as future work only. |

**Recommendation:** Stay with inline for v1. If daemon mode is added later, extract the web crate but keep the same `AppState` shape to minimize rework.

## 2. Schema management

| Approach | Pros | Cons | Plan decision |
|---|---|---|---|
| **Inline `CREATE TABLE IF NOT EXISTS`** (chosen) | Matches existing `db/mod.rs` pattern; no migration runner to maintain; works on first run. | Schema drift between `db/mod.rs` and any `migrations/` files; no downgrade path; harder to review schema history. | Chosen to match existing repo pattern. |
| **Add `sqlx::migrate!`** | Standard Rust migration workflow; schema history in `migrations/`; reversible with care. | Requires adding migration runner at startup; existing inline schema must be reconciled or removed. | Out of scope for this plan. |
| **Hybrid: migrations + baseline guard** | Best of both worlds for ops users. | More code and more chances for drift. | Not proposed. |

**Recommendation:** Use inline schema for v1, but add a comment block in `db/mod.rs` warning that `migrations/` are not applied at runtime and must be kept in sync manually.

## 3. Webhook secret strategy

| Approach | Pros | Cons | Plan decision |
|---|---|---|---|
| **Per-app HMAC secret** (chosen) | Blast-radius control; a leaked secret cannot trigger other apps; matches GitHub/GitLab per-repo webhooks. | Receiver must iterate candidates and validate each signature. | Chosen. |
| **Single global secret** | Simpler receiver logic; one env var to configure. | One leak compromises every app; harder rotation; conflicts with multi-tenant mental model. | Rejected. |
| **Global secret + per-app override** | Flexible; falls back to global. | Increases complexity and risk; users may rely on global secret. | Not proposed. |

**Recommendation:** Keep per-app secrets. Document that the receiver matches candidates by repo URL but authorizes by HMAC only.

## 4. Frontend framework

| Approach | Pros | Cons | Plan decision |
|---|---|---|---|
| **SvelteKit + adapter-static SPA** (chosen) | File-based routing; static output; single `index.html` fallback; familiar to existing `gh-pages/` work. | Needs Vite/SvelteKit toolchain; SPA mode requires careful base path and API URL handling. | Chosen. |
| **Plain Vite + Svelte 5** | Simpler build; direct control over routing; smaller config surface. | Manual router needed; more boilerplate for nested routes. | Mentioned earlier in the plan but superseded. |
| **Rust TUI embedded in browser** (e.g., ratatui via wasm) | Single language; reuse TUI screens. | Experimental; poor browser integration; huge wasm size; not practical. | Not proposed. |

**Recommendation:** SvelteKit static SPA is the right balance. Ensure `adapter-static` `fallback: 'index.html'` is set so nested `/ui/apps/:id/*` routes work.

## 5. Runtime backend dispatch

| Approach | Pros | Cons | Plan decision |
|---|---|---|---|
| **Trait-based dispatch** (`RuntimeBackend`) (chosen) | Type-safe; easy to add backends; testable; matches existing `core/` architecture. | Slightly more boilerplate than an enum; async trait requires `async-trait` or RPITIT. | Chosen. |
| **Enum with big match statements** | Simpler for a small fixed set; no trait objects. | Adding a backend touches many files; harder to unit test; less extensible. | Rejected. |
| **Plugin registry (WASM or dylib)** | Third-party backends without recompiling. | Major complexity; security review required; overkill for v1. | Listed as future WASM plugin idea. |

**Recommendation:** Trait-based dispatch. Define `supports_http_health()` in the trait so non-HTTP backends can opt out cleanly.

## 6. App metrics storage

| Approach | Pros | Cons | Plan decision |
|---|---|---|---|
| **In-memory ring buffer** (chosen) | No SQLite write amplification; constant memory per app; simple sparklines; survives WAL flush concerns. | Metrics lost on restart; no historical query API; bounded to one hour window. | Chosen for live monitoring. |
| **SQLite table with 1-second primary key** | Persistent; historical queries possible. | High write volume; primary key collisions when two samples share `datetime('now')`; WAL pressure. | Explicitly rejected in plan. |
| **Round-robin file (RRD style)** | Persistent; fixed size; standard metrics pattern. | Adds dependency or custom format; overkill for v1. | Not proposed. |

**Recommendation:** Keep the ring buffer. Expose Prometheus `/metrics` from memory and document that restart resets live app metrics.

## 7. Asset embedding strategy

| Approach | Pros | Cons | Plan decision |
|---|---|---|---|
| **Commit placeholder + `debug-embed`** (chosen) | `make check`/`make build` work without npm; release builds embed real assets. | Placeholder file must be kept in repo; devs must remember to run `npm run build` before release. | Chosen. |
| **Always embed real assets** | No placeholder; consistent binary. | Breaks `make check` on fresh clones without frontend build; adds npm dependency to every compile. | Rejected. |
| **Serve assets from disk in dev, embed in release** | Fast dev iteration; clean release. | Two code paths; runtime config complexity. | Partially what `debug-embed` achieves. |

**Recommendation:** Placeholder + `debug-embed`. Update `cli/Cargo.toml` to include `rust-embed` with `debug-embed` feature.

## 8. Zero-downtime deploy model

| Approach | Pros | Cons | Plan decision |
|---|---|---|---|
| **Temp port + Caddy route swap** (chosen) | Works with existing CaddyManager; no extra proxy layer; supports docker-compose and k3s. | Requires free temp port probing; orphan cleanup on gateway failure; not usable for systemd/PM2. | Chosen for docker-compose/k3s. |
| **Blue/green with separate network namespace** | Cleaner isolation; no port collision. | More complex; not supported by all backends. | Not proposed. |
| **External reverse proxy (Traefik labels, etc.)** | Backend-native zero downtime. | Adds dependency; conflicts with Caddy as gateway. | Rejected to keep single-gateway design. |

**Recommendation:** Temp-port swap for docker-compose/k3s only; accept restart downtime for systemd/PM2/static backends.
