use anyhow::Result;
use sqlx::{sqlite::SqliteConnectOptions, SqlitePool};
use std::str::FromStr;

pub mod agent_tasks;
pub mod audit;
pub mod deployments;

pub async fn init_db(db_url: &str) -> Result<SqlitePool> {
    let options = SqliteConnectOptions::from_str(db_url)?.create_if_missing(true);
    let pool = SqlitePool::connect_with(options).await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS audit_log (
            id      INTEGER PRIMARY KEY AUTOINCREMENT,
            action  TEXT NOT NULL,
            target  TEXT,
            output  TEXT,
            success INTEGER NOT NULL DEFAULT 1,
            ts      INTEGER NOT NULL
        )",
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS deployments (
            id           TEXT PRIMARY KEY,
            repo_url     TEXT NOT NULL,
            path         TEXT NOT NULL,
            deploy_type  TEXT NOT NULL,
            status       TEXT NOT NULL,
            last_updated TEXT NOT NULL
        )",
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS agent_tasks (
            id           INTEGER PRIMARY KEY AUTOINCREMENT,
            name         TEXT    NOT NULL,
            prompt       TEXT    NOT NULL,
            schedule     TEXT    NOT NULL,
            enabled      INTEGER NOT NULL DEFAULT 1,
            last_run_at  INTEGER,
            last_result  TEXT,
            last_success INTEGER,
            created_at   INTEGER NOT NULL DEFAULT (unixepoch())
        )",
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS projects_config (
            key   TEXT PRIMARY KEY,
            value TEXT NOT NULL
        )",
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        "INSERT OR IGNORE INTO projects_config (key, value) VALUES ('projects_dir', '~/projects')",
    )
    .execute(&pool)
    .await?;

    Ok(pool)
}
