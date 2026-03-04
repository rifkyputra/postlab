use anyhow::Result;
use sqlx::SqlitePool;

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
