use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Server {
    pub id: String,
    pub name: String,
    pub host: String,
    pub port: i64,
    pub user: String,
    pub auth_method: String,
    pub ssh_key_path: Option<String>,
    pub os_family: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Task {
    pub id: String,
    pub server_id: String,
    pub kind: String,
    pub status: String,
    pub input_json: Option<String>,
    pub output: Option<String>,
    pub error: Option<String>,
    pub created_at: String,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct AppInstallation {
    pub id: String,
    pub server_id: String,
    pub app_name: String,
    pub version: Option<String>,
    pub installed_at: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct AuditLog {
    pub id: String,
    pub server_id: Option<String>,
    pub task_id: Option<String>,
    pub action: String,
    pub details: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Config {
    pub key: String,
    pub value: String,
    pub updated_at: String,
}

// ── Input types for creating records ─────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct CreateServer {
    pub name: String,
    pub host: String,
    pub port: Option<i64>,
    pub user: Option<String>,
    pub auth_method: Option<String>,
    pub ssh_key_path: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateTask {
    pub server_id: String,
    pub kind: String,
    pub input_json: Option<serde_json::Value>,
}
