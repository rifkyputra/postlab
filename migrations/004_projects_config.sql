CREATE TABLE IF NOT EXISTS projects_config (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

INSERT OR IGNORE INTO projects_config (key, value) VALUES ('projects_dir', '~/projects');
