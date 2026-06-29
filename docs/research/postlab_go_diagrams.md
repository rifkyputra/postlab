# postlab go — Architecture and Flow Diagrams

Source: `docs/plan/postlab_go.md`

## 1. Overall architecture

```mermaid
graph TB
    subgraph "Binary: postlab"
        CLI[CLI parser: postlab go]
        WEB[axum web server]
        STATE[AppState<br/>db + platform + app_manager + ws_registry]
        AUTH[Auth middleware<br/>Bearer token + Origin/Host allow-list]
        ASSETS[RustEmbed<br/>web/dist]

        subgraph "core/ modules"
            GIT[git clone/pull --ff-only]
            DET[detector]
            BACK[RuntimeBackend trait]
            GATE[CaddyManager]
            SYS[system adapters]
        end

        subgraph "Runtime backends"
            DC[docker-compose]
            PM2[pm2]
            SD[systemd]
            K3[k3s]
            WC[wasmcloud]
            ST[static]
        end
    end

    DB[(SQLite)]
    CADDY[Caddy]
    DOCKER[Docker]
    SYSTEMD[systemd]
    KUBE[k3s/kubectl]
    BROWSER[Browser]

    CLI --> WEB
    WEB --> AUTH
    AUTH --> STATE
    WEB --> ASSETS
    BROWSER --> WEB

    STATE --> DB
    STATE --> GIT
    STATE --> DET
    STATE --> BACK
    BACK --> DC & PM2 & SD & K3 & WC & ST
    BACK --> GATE
    GATE --> CADDY
    DC --> DOCKER
    SD --> SYSTEMD
    K3 --> KUBE
```

## 2. Deploy workflow

```mermaid
flowchart LR
    A[git clone/pull<br/>--ff-only] --> B[detect runtime]
    B --> C[build]
    C --> D[generate config]
    D --> E[start / zero-downtime start]
    E --> F{health check?}
    F -->|success| G[update Caddy gateway]
    F -->|failure| H[mark failed<br/>keep old version]
    G --> I[status: running<br/>write deploy log]
    H --> J[emit deploy.complete failed]
    I --> K[emit deploy.complete running]

    A -.->|deploy.progress| WS[WebSocket]
    C -.->|deploy.progress| WS
    E -.->|deploy.progress| WS
    F -.->|deploy.progress| WS
    G -.->|deploy.progress| WS
```

## 3. API authentication flow

```mermaid
sequenceDiagram
    participant B as Browser
    participant S as axum server
    participant A as Auth middleware
    participant R as Route handler
    participant DB as SQLite

    B->>S: GET /api/v1/apps<br/>Authorization: Bearer <token><br/>Origin: http://127.0.0.1:9020
    S->>A: check Origin/Host allow-list
    alt Origin/Host rejected
        A-->>B: 403 Forbidden
    else Allowed
        A->>DB: fetch stored hash
        A->>A: hash submitted token<br/>compare to stored hash
        alt Invalid token
            A-->>B: 401 Unauthorized
        else Valid
            A->>R: continue
            R-->>B: JSON response
        end
    end
```

## 4. Webhook receiver flow

```mermaid
sequenceDiagram
    participant GH as GitHub/GitLab
    participant WH as Webhook server<br/>0.0.0.0:9021
    participant DB as SQLite
    participant AM as AppManager

    GH->>WH: POST /webhooks/github<br/>X-GitHub-Delivery: <id><br/>X-Hub-Signature-256: <hmac>
    WH->>DB: SELECT apps WHERE repo_url = payload.url
    WH->>WH: Build candidate set
    loop Each candidate
        WH->>WH: Compute HMAC with candidate.webhook_secret
        alt Signature matches
            WH->>DB: Check delivery ID not seen (TTL 24h)
            alt Duplicate or timestamp skew
                WH-->>GH: 401/409
            else Fresh
                WH->>AM: tokio::spawn deploy(app_id, sha, msg)
                WH-->>GH: 202 Accepted
            end
        end
    end
    WH-->>GH: 401 if no candidate matched
```

## 5. Zero-downtime deploy sequence (docker-compose / k3s)

```mermaid
sequenceDiagram
    participant AM as AppManager
    participant BE as RuntimeBackend
    participant HC as Health checker
    participant GW as CaddyManager
    participant OLD as Old container/pod
    participant NEW as New container/pod

    AM->>BE: start new version on temp port
    BE->>NEW: docker compose up -d (temp port)
    AM->>HC: poll temp port /health_path
    HC-->>AM: 200 OK
    AM->>GW: add_route domain -> localhost:temp_port
    alt Gateway update succeeds
        GW-->>AM: route swapped
        AM->>BE: stop old version
        BE->>OLD: docker compose down
        AM->>AM: mark running
    else Gateway update fails
        AM->>BE: stop new version
        BE->>NEW: docker compose down
        AM->>AM: mark failed, keep old route
    end
```

## 6. Data model

```mermaid
erDiagram
    apps ||--o{ app_env_vars : has
    apps ||--o{ app_deploys : has

    apps {
        TEXT id PK
        TEXT name
        TEXT runtime
        TEXT language
        TEXT repo_url
        TEXT repo_branch
        TEXT domain
        INTEGER port
        TEXT health_path
        TEXT status
        TEXT webhook_secret
        TEXT config_dir
        TEXT created_at
        TEXT updated_at
    }

    app_env_vars {
        TEXT app_id FK
        TEXT key
        TEXT value
        INTEGER secret
    }

    app_deploys {
        TEXT id PK
        TEXT app_id FK
        TEXT commit_sha
        TEXT commit_msg
        TEXT status
        TEXT started_at
        TEXT finished_at
        TEXT log_ref
    }
```

## 7. Port allocation fallback

```mermaid
flowchart TD
    START[postlab go starts] --> BIND_API{Bind API port}
    BIND_API -->|default 9020 in use| SKIP1[Skip 9021/9022<br/>probe 9023+]
    BIND_API -->|explicit port in use| ERR1[Exit with error]
    BIND_API -->|free| NEXT{Bind metrics port}
    SKIP1 --> NEXT
    NEXT -->|default 9022 in use| SKIP2[Skip 9020/9021<br/>probe 9041+]
    NEXT -->|explicit port in use| ERR2[Exit with error]
    NEXT -->|free| WH{--webhook?}
    SKIP2 --> WH
    WH -->|yes| WH_BIND{Bind webhook port}
    WH_BIND -->|default 9021 in use| SKIP3[Skip 9020/9022<br/>probe 9031+]
    WH_BIND -->|explicit port in use| ERR3[Exit with error]
    WH -->|no| RUN[Run servers]
    SKIP3 --> RUN
    WH_BIND -->|free| RUN
```
