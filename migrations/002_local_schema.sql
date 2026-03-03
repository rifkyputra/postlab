-- Migration 002: remove server references, local-execution model
-- Rebuild tasks without server_id
ALTER TABLE tasks RENAME TO _tasks_old;
CREATE TABLE tasks (
    id           TEXT PRIMARY KEY,
    kind         TEXT NOT NULL,
    status       TEXT NOT NULL DEFAULT 'pending',
    input_json   TEXT,
    output       TEXT,
    error        TEXT,
    created_at   TEXT NOT NULL,
    started_at   TEXT,
    completed_at TEXT
);
INSERT INTO tasks (id, kind, status, input_json, output, error, created_at, started_at, completed_at)
    SELECT id, kind, status, input_json, output, error, created_at, started_at, completed_at FROM _tasks_old;
DROP TABLE _tasks_old;

-- Rebuild app_installations without server_id
ALTER TABLE app_installations RENAME TO _ai_old;
CREATE TABLE app_installations (
    id           TEXT PRIMARY KEY,
    app_name     TEXT NOT NULL,
    version      TEXT,
    installed_at TEXT NOT NULL,
    status       TEXT NOT NULL
);
INSERT INTO app_installations (id, app_name, version, installed_at, status)
    SELECT id, app_name, version, installed_at, status FROM _ai_old;
DROP TABLE _ai_old;

-- Rebuild audit_log without server_id
ALTER TABLE audit_log RENAME TO _al_old;
CREATE TABLE audit_log (
    id         TEXT PRIMARY KEY,
    task_id    TEXT,
    action     TEXT NOT NULL,
    details    TEXT,
    created_at TEXT NOT NULL
);
INSERT INTO audit_log (id, task_id, action, details, created_at)
    SELECT id, task_id, action, details, created_at FROM _al_old;
DROP TABLE _al_old;

-- Drop servers table
DROP TABLE IF EXISTS servers;
