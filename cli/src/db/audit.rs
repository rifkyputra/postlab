use anyhow::Result;
use sqlx::{FromRow, SqlitePool};

#[derive(Debug, Clone, FromRow)]
pub struct AuditEntry {
    pub id: i64,
    pub action: String,
    pub target: Option<String>,
    pub output: Option<String>,
    pub success: bool,
    pub ts: i64,
}

pub async fn log_action(
    pool: &SqlitePool,
    action: &str,
    target: Option<&str>,
    output: &str,
    success: bool,
) -> Result<()> {
    let ts = chrono::Utc::now().timestamp();
    sqlx::query(
        "INSERT INTO audit_log (action, target, output, success, ts) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(action)
    .bind(target)
    .bind(output)
    .bind(success as i64)
    .bind(ts)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn recent(pool: &SqlitePool, limit: u32) -> Result<Vec<AuditEntry>> {
    let rows = sqlx::query_as::<_, AuditEntry>(
        "SELECT id, action, target, output, success, ts FROM audit_log ORDER BY ts DESC LIMIT ?",
    )
    .bind(limit as i64)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}
