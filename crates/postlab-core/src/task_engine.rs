use anyhow::Result;
use chrono::Utc;
use serde_json::Value;
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::models::Task;

pub struct TaskEngine {
    pool: SqlitePool,
}

impl TaskEngine {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Create a task record and enqueue it for async execution.
    /// Returns the new task ID.
    pub async fn spawn_task(
        &self,
        server_id: &str,
        kind: &str,
        input: Option<Value>,
    ) -> Result<String> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        let input_json = input.map(|v| v.to_string());

        sqlx::query(
            "INSERT INTO tasks (id, server_id, kind, status, input_json, created_at)
             VALUES (?, ?, ?, 'pending', ?, ?)",
        )
        .bind(&id)
        .bind(server_id)
        .bind(kind)
        .bind(&input_json)
        .bind(&now)
        .execute(&self.pool)
        .await?;

        // Spawn async execution (stub — actual module dispatch added per feature)
        let pool = self.pool.clone();
        let task_id = id.clone();
        tokio::spawn(async move {
            if let Err(e) = run_task(&pool, &task_id).await {
                tracing::error!("Task {task_id} failed: {e}");
            }
        });

        Ok(id)
    }

    pub async fn get_task(&self, id: &str) -> Result<Option<Task>> {
        let task = sqlx::query_as::<_, Task>("SELECT * FROM tasks WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(task)
    }

    pub async fn list_tasks(&self, server_id: Option<&str>) -> Result<Vec<Task>> {
        let tasks = match server_id {
            Some(sid) => {
                sqlx::query_as::<_, Task>(
                    "SELECT * FROM tasks WHERE server_id = ? ORDER BY created_at DESC",
                )
                .bind(sid)
                .fetch_all(&self.pool)
                .await?
            }
            None => {
                sqlx::query_as::<_, Task>("SELECT * FROM tasks ORDER BY created_at DESC")
                    .fetch_all(&self.pool)
                    .await?
            }
        };
        Ok(tasks)
    }
}

async fn run_task(pool: &SqlitePool, task_id: &str) -> Result<()> {
    let now = Utc::now().to_rfc3339();

    // Mark running
    sqlx::query("UPDATE tasks SET status = 'running', started_at = ? WHERE id = ?")
        .bind(&now)
        .bind(task_id)
        .execute(pool)
        .await?;

    // TODO: dispatch to module based on task.kind
    // For now, succeed immediately as a stub
    let done = Utc::now().to_rfc3339();
    sqlx::query(
        "UPDATE tasks SET status = 'success', output = 'stub: not yet implemented', completed_at = ? WHERE id = ?",
    )
    .bind(&done)
    .bind(task_id)
    .execute(pool)
    .await?;

    Ok(())
}
