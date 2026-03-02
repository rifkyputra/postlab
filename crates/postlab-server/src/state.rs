use postlab_core::task_engine::TaskEngine;
use sqlx::SqlitePool;

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub engine: std::sync::Arc<TaskEngine>,
}

impl AppState {
    pub fn new(pool: SqlitePool) -> Self {
        let engine = std::sync::Arc::new(TaskEngine::new(pool.clone()));
        Self { pool, engine }
    }
}
