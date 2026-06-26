CREATE TABLE IF NOT EXISTS agent_tasks (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    name         TEXT    NOT NULL,
    prompt       TEXT    NOT NULL,
    schedule     TEXT    NOT NULL,
    enabled      INTEGER NOT NULL DEFAULT 1,
    last_run_at  INTEGER,
    last_result  TEXT,
    last_success INTEGER,
    created_at   INTEGER NOT NULL DEFAULT (unixepoch())
);
