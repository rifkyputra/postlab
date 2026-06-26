use anyhow::Result;
use sqlx::SqlitePool;

#[derive(Debug, Clone)]
pub struct AgentTask {
    pub id: i64,
    pub name: String,
    pub prompt: String,
    pub schedule: String,
    pub enabled: bool,
    pub last_run_at: Option<i64>,
    #[allow(dead_code)]
    pub last_result: Option<String>,
    pub last_success: Option<bool>,
    #[allow(dead_code)]
    pub created_at: i64,
}

pub const SCHEDULE_OPTIONS: &[&str] = &["30m", "1h", "6h", "12h", "24h"];

pub fn schedule_secs(s: &str) -> i64 {
    match s {
        "30m" => 1800,
        "1h" => 3600,
        "6h" => 21600,
        "12h" => 43200,
        "24h" => 86400,
        _ => 3600,
    }
}

pub async fn list_tasks(pool: &SqlitePool) -> Result<Vec<AgentTask>> {
    let rows = sqlx::query_as::<_, (i64, String, String, String, i64, Option<i64>, Option<String>, Option<i64>, i64)>(
        "SELECT id, name, prompt, schedule, enabled, last_run_at, last_result, last_success, created_at
         FROM agent_tasks ORDER BY created_at DESC",
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|(id, name, prompt, schedule, enabled, last_run_at, last_result, last_success, created_at)| {
            AgentTask {
                id,
                name,
                prompt,
                schedule,
                enabled: enabled != 0,
                last_run_at,
                last_result,
                last_success: last_success.map(|v| v != 0),
                created_at,
            }
        })
        .collect())
}

pub async fn create_task(pool: &SqlitePool, name: &str, prompt: &str, schedule: &str) -> Result<AgentTask> {
    let now = chrono::Utc::now().timestamp();
    let id = sqlx::query(
        "INSERT INTO agent_tasks (name, prompt, schedule, created_at) VALUES (?, ?, ?, ?)",
    )
    .bind(name)
    .bind(prompt)
    .bind(schedule)
    .bind(now)
    .execute(pool)
    .await?
    .last_insert_rowid();

    Ok(AgentTask {
        id,
        name: name.to_string(),
        prompt: prompt.to_string(),
        schedule: schedule.to_string(),
        enabled: true,
        last_run_at: None,
        last_result: None,
        last_success: None,
        created_at: now,
    })
}

pub async fn delete_task(pool: &SqlitePool, id: i64) -> Result<()> {
    sqlx::query("DELETE FROM agent_tasks WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn toggle_task(pool: &SqlitePool, id: i64, enabled: bool) -> Result<()> {
    sqlx::query("UPDATE agent_tasks SET enabled = ? WHERE id = ?")
        .bind(enabled as i64)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn mark_run(pool: &SqlitePool, id: i64, result: &str, success: bool) -> Result<()> {
    let now = chrono::Utc::now().timestamp();
    sqlx::query(
        "UPDATE agent_tasks SET last_run_at = ?, last_result = ?, last_success = ? WHERE id = ?",
    )
    .bind(now)
    .bind(result)
    .bind(success as i64)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}
