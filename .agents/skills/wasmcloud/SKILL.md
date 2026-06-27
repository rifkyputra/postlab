---
name: wasmcloud
description: wasmCloud application development and deployment. Use when the user asks to build, deploy, or debug wasmCloud components, write WIT worlds, link components to providers, configure wadm manifests, use the wash CLI, create TypeScript/Rust/Go Wasm components, or understand the wasmCloud lattice. Covers wash, wadm, WIT, jco, ComponentizeJS, Hono, providers, and the wasmCloud runtime.
---

# wasmCloud

wasmCloud is a cloud-native platform for running WebAssembly workloads across any cloud, Kubernetes, datacenter, or edge. Components are lightweight, secure-by-default, and composed dynamically at runtime.

## Execution Model

Pi executes tool calls sequentially, even when you emit multiple calls in one turn. Batch independent calls in a single turn to save round-trips:

| Pattern | Use for |
|---------|---------|
| Multiple bash calls in one turn | Check NATS status + wash up + curl test |
| `wash` commands via bash | Build, inspect, link, deploy components |

## Architecture

```
┌─────────────────────────────────────────────────┐
│  Workload = Components + optional Service       │
│  ┌──────────┐  wRPC/NATS  ┌──────────┐         │
│  │Component │◄───────────►│ Provider  │         │
│  │(imports) │              │(exports)  │         │
│  └──────────┘              └──────────┘         │
│         ↕                        ↕              │
│  ┌──────────┐              ┌──────────┐         │
│  │Component │              │  Redis   │         │
│  │ (logic)  │              │  (DB)    │         │
│  └──────────┘              └──────────┘         │
│                                                     │
│  Lattice = self-forming NATS mesh                  │
└─────────────────────────────────────────────────────┘
```

| Concept | What it is |
|---------|------------|
| **Component** | Stateless `.wasm` binary implementing business logic. Declares imports (what it needs) and exports (what it provides) via WIT. |
| **Service** | Stateful, long-running Wasm binary that can listen on TCP ports, maintain connections, survive restarts. |
| **Provider** | Host plugin exporting a WASI interface — bridges components to external resources (Redis, HTTP, databases, etc.). Runs as a process on a host. |
| **Host** | The runtime environment (`wash up` or embedded `wash-runtime`). Executes components and providers on Wasmtime. |
| **Lattice** | Self-forming, self-healing mesh over NATS. Components and providers communicate over it via wRPC. Flat topology across any infrastructure. |
| **Link** | A declared connection between an importer and exporter at a specific WASI interface. Unidirectional. Defined in wadm manifests. Can be changed at runtime without redeployment. |
| **wRPC** | wasmCloud's RPC protocol. Translates WASI interface calls into NATS messages for dynamic composition. |
| **wadm** | Application Deployment Manager. Reconciles declarative manifests to desired state. Like a K8s controller for wasmCloud. |
| **WIT** | WebAssembly Interface Types — the IDL for declaring component imports and exports. |

## CLI: `wash`

### Setup

```bash
wash up                          # Start a local wasmCloud host (NATS + wadm + host)
wash up --detached               # Background daemon
wash down                        # Stop the host
wash status                      # Check host status
```

### Project Scaffolding

```bash
# TypeScript component from template
wash new component my-component --template-name hello-world-typescript
wash new https://github.com/wasmCloud/typescript.git --subfolder templates/http-hello-world-fetch

# Hono-based HTTP handler (recommended)
wash new https://github.com/wasmCloud/typescript.git --subfolder templates/http-handler-hono

# Rust component
wash new component my-component --template-name hello-world-rust

# Go component
wash new component my-component --template-name hello-world-tinygo

# Python component
wash new component my-component --template-name hello-world-python
```

### Build

```bash
wash build                       # Run the build command defined in .wash/config.yaml
cd my-project && npm run build  # TypeScript: tsc + jco componentize
cd my-project && cargo build    # Rust
```

### Registry

```bash
wash push <oci-ref>              # Push component to OCI registry (e.g., ghcr.io/user/comp:0.1.0)
wash pull <oci-ref>              # Pull component from registry
wash inspect <oci-ref>           # Inspect a component's WIT interfaces
```

### Linking (Imperative)

```bash
wash link put <source> <target> <namespace> <package> <interface> [--name <name>]
wash link del <source> <target> <namespace> <package> <interface>
wash link query                  # List all active links
```

### Application Deployment

```bash
wash app deploy wadm.yaml        # Deploy declarative app manifest
wash app undeploy <app-name>     # Tear down
wash app list                    # List running applications
wash app status <app-name>       # Check deployment status
wash app delete <app-name>       # Remove from wadm
```

### Component Inspection

```bash
wash get inventory <host-id>     # List all components on a host
wash get hosts                   # List all connected hosts
wash get claims <component-ref>  # Inspect a component's imports and exports
```

## WIT Worlds

WIT files live in `wit/world.wit`. They declare what a component imports and exports:

```wit
package wasmcloud:hello;

world hello {
    import wasi:http/outgoing-handler@0.2.3;     // Allows fetch() calls
    import wasi:keyvalue/store@0.2.0-draft;      // Allows key-value ops
    import wasi:logging/logging@0.1.0-draft;     // Allows logging
    export wasi:http/incoming-handler@0.2.3;     // Handles incoming HTTP
}
```

Common WASI interfaces:

| Interface | Version | Purpose |
|-----------|---------|---------|
| `wasi:http/incoming-handler` | 0.2.3 | Receive HTTP requests |
| `wasi:http/outgoing-handler` | 0.2.3 | Make HTTP requests (powers `fetch()`) |
| `wasi:keyvalue/store` | 0.2.0-draft | Key-value storage |
| `wasi:keyvalue/atomics` | 0.2.0-draft | Atomic KV operations |
| `wasi:blobstore/blobstore` | 0.2.0-draft | Object/blob storage |
| `wasi:logging/logging` | 0.1.0-draft | Structured logging |
| `wasi:config/runtime` | 0.2.0-draft | Runtime configuration injection |
| `wasi:cli/run` | 0.2.2 | CLI-style execution (for services) |
| `wasmcloud:messaging/consumer` | 0.3.0-draft | Message queue consumption |

## TypeScript Component Development

### Build Pipeline

```
TypeScript source (.ts)
    → tsc → JavaScript (.js)
    → bundler (optional, for npm deps) → bundled.js
    → jco componentize → .wasm component
```

Key tools:

| Tool | Role |
|------|------|
| `jco` | JavaScript component toolchain — `guest-types`, `componentize`, `transpile` |
| `ComponentizeJS` | Library behind `jco componentize` — embeds SpiderMonkey in the Wasm |
| `StarlingMonkey` | SpiderMonkey compiled to Wasm — provides Web APIs inside the component |

### Project Structure

```
my-component/
├── .wash/config.yaml         # Build command + component output path
├── wit/world.wit             # WIT world definition
├── generated/types/          # jco guest-types output (auto-generated)
├── src/index.ts              # Application code
├── dist/                     # Build output (.wasm binary)
├── package.json              # npm scripts for build pipeline
└── tsconfig.json             # Maps WASI import specifiers to type defs
```

### `.wash/config.yaml`

```yaml
build:
  command: npm run install-and-build
  component_path: dist/http-hello-world.wasm
```

### `package.json` Build Scripts

```json
{
  "scripts": {
    "generate:types": "jco guest-types wit/ -o generated/types",
    "build:ts": "tsc",
    "build:js": "jco componentize -w wit -o dist/my-component.wasm dist/my-component.js",
    "build": "npm run generate:types && npm run build:ts && npm run build:js",
    "install-and-build": "npm install && npm run build"
  },
  "devDependencies": {
    "@bytecodealliance/jco": "^1.16.0",
    "typescript": "^5.9.0"
  }
}
```

### Two HTTP Patterns

**Service Worker fetch pattern** (recommended — works with Hono):

```typescript
addEventListener('fetch', (event) => {
    event.respondWith(new Response('Hello from TypeScript!\n'));
});
```

**Direct incomingHandler export** (lower-level, more control):

```typescript
import { IncomingRequest, ResponseOutparam, OutgoingBody, OutgoingResponse, Fields } from 'wasi:http/types@0.2.3';

function handle(req: IncomingRequest, resp: ResponseOutparam) {
    const response = new OutgoingResponse(new Fields());
    // ... construct response ...
    ResponseOutparam.set(resp, { tag: 'ok', val: response });
}

export const incomingHandler = { handle };
```

### Hono Framework Integration (Recommended)

```typescript
import { Hono } from 'hono';
import { fire } from '@bytecodealliance/jco-std/wasi/0.2.6/http/adapters/hono/server';

const app = new Hono();

app.get('/', (c) => c.text('Hello from wasmCloud!'));
app.get('/api/data', (c) => c.json({ message: 'data' }));

fire(app);

export { incomingHandler } from '@bytecodealliance/jco-std/wasi/0.2.6/http/adapters/hono/server';
```

### Using WASI Interfaces in TypeScript

```typescript
// Logging
import { log, Level } from 'wasi:logging/logging@0.1.0-draft';
log(Level.Info, '', 'Hello from logs');

// Outgoing HTTP (also available as standard fetch())
const response = await fetch('https://api.example.com/data');
const data = await response.json();

// Key-value storage
import * as kv from 'wasi:keyvalue/store@0.2.0-draft';
kv.set('my-bucket', 'my-key', new TextEncoder().encode('value'));
const bytes = kv.get('my-bucket', 'my-key');

// Configuration
import { get } from 'wasi:config/runtime@0.2.0-draft';
const dbUrl = get('database_url');
```

### Library Compatibility

**Works**: Hono, itty-router, Axios (`fetch()` mode), Zod, date-fns, lodash (pure), any Web Standards library.

**Does NOT work**: Express.js, Fastify (`http` module), `pg`/`mysql2` (Node.js `net`), `fs`, `path`, `process.env`, `Buffer`, `require()`, native addons.

## wadm Manifests

### Basic HTTP Component

```yaml
apiVersion: core.oam.dev/v1beta1
kind: Application
metadata:
  name: hello-world
  annotations:
    version: v0.1.0
    description: 'HTTP hello world'
spec:
  components:
    - name: http-component
      type: component
      properties:
        image: file://./build/http_hello_world_s.wasm
      traits:
        - type: spreadscaler
          properties:
            instances: 1
        - type: link
          properties:
            target: kvredis
            namespace: wasi
            package: keyvalue
            interfaces: [atomics, store]
            target_config:
              - name: redis-url
                properties:
                  url: redis://127.0.0.1:6379

    - name: kvredis
      type: capability
      properties:
        image: ghcr.io/wasmcloud/keyvalue-redis:0.28.2

    - name: httpserver
      type: capability
      properties:
        image: ghcr.io/wasmcloud/http-server:0.26.0
      traits:
        - type: link
          properties:
            target: http-component
            namespace: wasi
            package: http
            interfaces: [incoming-handler]
            source_config:
              - name: default-http
                properties:
                  address: 0.0.0.0:8000
```

### Link Direction Rule

**Links are always defined as a trait of the source (importer).** The source is the one that needs something; the target is the one that provides it.

```
Component imports wasi:keyvalue → link is on the component, target is the provider
Provider imports wasi:http     → link is on the provider, target is the component
```

### Named Links (Multiple Configurations of Same Interface)

```yaml
- type: link
  properties:
    name: local-cache       # Link name
    target: kvredis
    namespace: wasi
    package: keyvalue
    interfaces: [store]
    target_config:
      - name: redis-url
        properties:
          url: redis://127.0.0.1:6379
- type: link
  properties:
    name: cloud-store       # Same interface, different config
    target: kvredis
    namespace: wasi
    package: keyvalue
    interfaces: [store]
    target_config:
      - name: redis-url
        properties:
          url: redis://cloud.example.com:6379
```

In code, select the active link by name:

```typescript
import { set_link_name, CallTargetInterface } from 'wasmcloud:bus/lattice@1.0.0';

const kvInterface = new CallTargetInterface('wasi', 'keyvalue', 'store');
set_link_name('local-cache', [kvInterface]);
// Now calls route to the local cache
```

### Image References

| Format | Example |
|--------|---------|
| Local file | `file://./build/my-component.wasm` |
| OCI registry | `ghcr.io/wasmcloud/http-server:0.26.0` |
| OCI with digest | `ghcr.io/wasmcloud/http-server@sha256:abc...` |

## Built-in Providers (No Link Needed)

These are part of every wasmCloud host and are auto-resolved when a component imports them:

| Interface | Provider |
|-----------|----------|
| `wasi:logging/logging` | Host logging system |
| `wasi:random/random` | Cryptographically secure random |
| `wasi:clocks/monotonic-clock` | Monotonic time |
| `wasi:clocks/wall-clock` | Wall clock time |

To use them, just add the import to your WIT world and call the interface — no link definition needed in the wadm manifest.

## Common Provider Images

| Provider | Image | Exports |
|----------|-------|---------|
| HTTP Server | `ghcr.io/wasmcloud/http-server:0.26.0` | Imports `wasi:http/incoming-handler` |
| HTTP Client | `ghcr.io/wasmcloud/http-client:0.12.0` | Exports `wasi:http/outgoing-handler` |
| Redis KV | `ghcr.io/wasmcloud/keyvalue-redis:0.28.2` | Exports `wasi:keyvalue` |
| NATS KV | `ghcr.io/wasmcloud/keyvalue-nats:0.3.0` | Exports `wasi:keyvalue` |
| Vault KV | `ghcr.io/wasmcloud/keyvalue-vault:0.5.0` | Exports `wasi:keyvalue` |
| FS Blobstore | `ghcr.io/wasmcloud/blobstore-fs:0.10.0` | Exports `wasi:blobstore` |
| S3 Blobstore | `ghcr.io/wasmcloud/blobstore-s3:0.3.0` | Exports `wasi:blobstore` |
| NATS Messaging | `ghcr.io/wasmcloud/messaging-nats:0.3.0` | Exports `wasmcloud:messaging` |
| Postgres | `ghcr.io/wasmcloud/wasmcloud-postgres:0.1.0` | Exports `wasmcloud:postgres` |
| SQL Postgres | `ghcr.io/wasmcloud/sqldb-postgres:0.1.0` | Exports `wasmcloud:sqldb` |

## Common Workflows

### "Create a new TypeScript HTTP API with Hono"

```bash
wash new https://github.com/wasmCloud/typescript.git --subfolder templates/http-handler-hono
cd http-handler-hono
npm install
npm run build
wash up
wash app deploy wadm.yaml
curl localhost:8000
```

### "Add key-value storage to a component"

1. Add to `wit/world.wit`:
   ```wit
   import wasi:keyvalue/store@0.2.0-draft;
   ```
2. Run `jco guest-types wit/ -o generated/types`
3. Use in code:
   ```typescript
   import * as kv from 'wasi:keyvalue/store@0.2.0-draft';
   kv.set('bucket', 'key', new TextEncoder().encode('value'));
   ```
4. Add provider + link to `wadm.yaml`

### "Deploy to a remote host"

```bash
wash push ghcr.io/myorg/my-component:0.1.0
# Update wadm.yaml image to the OCI reference
wash app deploy wadm.yaml
```

### "Debug a component"

```bash
# Check host status
wash get hosts
wash get inventory <host-id>

# Check links
wash link query

# Check application status
wash app status <app-name>

# View logs (if wasi:logging is imported)
# Logs appear in the wash up terminal or systemd journal
```

### "Update a component without downtime"

```bash
wash push ghcr.io/myorg/my-component:0.2.0
# Update the image reference in wadm.yaml
wash app deploy wadm.yaml   # wadm reconciles, rolling update
```

### "Swap a provider at runtime"

```bash
# Change link from Redis-based KV to NATS-based KV
wash link del http-component kvredis wasi keyvalue store
wash link put http-component kvnats wasi keyvalue store
```

### "Port an Express.js backend to wasmCloud"

This requires rewriting the HTTP layer. Steps:

1. Replace Express with Hono (similar API, Web Standards-based):
   - `req.query` → `c.req.query()`
   - `req.body` → `c.req.json()`
   - `res.json()` → `c.json()`
   - `app.use(middleware)` → `app.use(middleware)` (identical)
2. Replace database drivers with provider links
3. Replace `process.env` with `wasi:config/runtime`
4. Extract pure business logic into shared modules
5. Add component entry point with `jco-std` Hono adapter

For dual Node.js + wasmCloud deployment, share the Hono app between both entry points:

```typescript
// src/app.ts — shared between both runtimes
import { Hono } from 'hono';
export const app = new Hono();
app.get('/', (c) => c.text('Hello!'));

// src/component.ts — wasmCloud entry
import { fire } from '@bytecodealliance/jco-std/wasi/0.2.6/http/adapters/hono/server';
import { app } from './app';
fire(app);
export { incomingHandler } from '@bytecodealliance/jco-std/wasi/0.2.6/http/adapters/hono/server';

// src/server.ts — Node.js entry
import { serve } from '@hono/node-server';
import { app } from './app';
serve({ fetch: app.fetch, port: 3000 });
```

### "Generate a wadm manifest from a component"

```bash
# Use wit2wadm to auto-generate manifest from WIT interfaces
npx wit2wadm --component build/my-component.wasm --output wadm.yaml
```

## Known Providers: Configuration Reference

### HTTP Server

```yaml
source_config:
  - name: default-http
    properties:
      address: 0.0.0.0:8000        # Bind address:port
```

### Redis Key-Value

```yaml
target_config:
  - name: redis-url
    properties:
      url: redis://127.0.0.1:6379   # Redis connection URL
```

### NATS Key-Value

```yaml
target_config:
  - name: nats-connection
    properties:
      cluster_uris: nats://127.0.0.1:4222
```

### S3 Blobstore

```yaml
target_config:
  - name: s3-config
    properties:
      endpoint: http://127.0.0.1:9000
      access_key_id: minioadmin
      secret_access_key: minioadmin
      region: us-east-1
      allow_http: true
```

### Filesystem Blobstore

```yaml
target_config:
  - name: fs-config
    properties:
      root_path: /tmp/wasmcloud-blobs
```

## Troubleshooting

| Symptom | Likely Cause | Fix |
|---------|-------------|-----|
| `wash build` fails with "command not found" | npm/node/jco not installed | `which npm`, install Node.js 18+ |
| `jco componentize` fails with missing export | WIT world version mismatch with jco | Check version strings match between `world.wit` and `jco` |
| Component starts but 404 on requests | Link not configured | `wash link query` — verify link connects httpserver to component |
| Provider fails to start | Missing config | Check `target_config`/`source_config` in link definition |
| `wash up` fails with port conflict | NATS port 4222 already in use | Kill existing NATS or use `--nats-port` |
| Component can't reach external service | Outgoing HTTP not imported | Add `wasi:http/outgoing-handler@0.2.3` to WIT world |
| TypeScript build error: cannot find module `wasi:*` | Missing `jco guest-types` or tsconfig paths | Run `npm run generate:types`, check `paths` in tsconfig |
| "Missing host interface implementation in the linker" on K8s | Host interface not declared in workload spec | Add to `spec.hostInterfaces` in WorkloadDeployment |

## Tips

- Use `wash up --detached` for long-running development; check `wash status`
- The `wit2wadm` tool auto-generates wadm manifests from component WIT interfaces
- Built-in providers (logging, random, clocks) need no link definitions
- `wash dev` provides a watch + rebuild + hot-reload development loop
- Component binaries include ~8MB of SpiderMonkey runtime (fixed cost, no per-code overhead)
- A component can have multiple named links to the same interface for different configurations
- Links are idempotent and can be created before or after the linked entities are running
- The lattice uses NATS queue subscriptions — scale components horizontally and NATS load-balances
- For Kubernetes: use `runtime-operator` with `WorkloadDeployment` CRDs instead of wadm manifests
- `jco transpile` converts a `.wasm` component back to JS runnable in Node.js — useful for backward compatibility
