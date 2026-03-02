CREATE TABLE servers (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL UNIQUE,
    host        TEXT NOT NULL,
    port        INTEGER NOT NULL DEFAULT 22,
    user        TEXT NOT NULL DEFAULT 'root',
    auth_method TEXT NOT NULL DEFAULT 'key',   -- 'key' or 'password'
    ssh_key_path TEXT,
    os_family   TEXT,                          -- 'ubuntu', 'fedora', or NULL (auto-detect)
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);

CREATE TABLE tasks (
    id           TEXT PRIMARY KEY,
    server_id    TEXT NOT NULL REFERENCES servers(id),
    kind         TEXT NOT NULL,    -- 'install_app', 'upgrade_os', 'harden', 'run_script', etc.
    status       TEXT NOT NULL DEFAULT 'pending', -- pending, running, success, failed
    input_json   TEXT,
    output       TEXT,
    error        TEXT,
    created_at   TEXT NOT NULL,
    started_at   TEXT,
    completed_at TEXT
);

CREATE TABLE app_installations (
    id           TEXT PRIMARY KEY,
    server_id    TEXT NOT NULL REFERENCES servers(id),
    app_name     TEXT NOT NULL,
    version      TEXT,
    installed_at TEXT NOT NULL,
    status       TEXT NOT NULL DEFAULT 'active'
);

CREATE TABLE audit_log (
    id        TEXT PRIMARY KEY,
    server_id TEXT REFERENCES servers(id),
    task_id   TEXT REFERENCES tasks(id),
    action    TEXT NOT NULL,
    details   TEXT,
    created_at TEXT NOT NULL
);

CREATE TABLE config (
    key        TEXT PRIMARY KEY,
    value      TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
