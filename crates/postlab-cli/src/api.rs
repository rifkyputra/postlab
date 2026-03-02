/// Typed HTTP client for the postlab-server API.
///
/// Server URL defaults to http://localhost:3000; override with POSTLAB_URL env var.
use anyhow::{Context, Result};
use postlab_core::models::{Server, Task};
use reqwest::Client;
use serde::{Deserialize, Serialize};

fn server_url() -> String {
    std::env::var("POSTLAB_URL").unwrap_or_else(|_| "http://localhost:3000".to_string())
}

fn client() -> Client {
    Client::new()
}

/// Wrap a reqwest send error with a human-friendly message.
/// Connection-refused errors get a "server not running?" hint.
fn send_err(e: reqwest::Error, url: &str) -> anyhow::Error {
    if e.is_connect() || e.is_timeout() {
        anyhow::anyhow!(
            "Cannot reach postlab-server at {url}\n  \
             → Start it with: make server  (or cargo run -p postlab-server)\n  \
             → Override URL:  POSTLAB_URL=http://host:port  {}\n  \
             → Cause: {e}",
            std::env::current_exe()
                .ok()
                .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
                .unwrap_or_else(|| "postlab".into())
        )
    } else {
        anyhow::anyhow!("HTTP request to {url} failed: {e}")
    }
}

// ── Servers ───────────────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct CreateServerInput {
    pub name: String,
    pub host: String,
    pub port: Option<i64>,
    pub user: Option<String>,
    pub auth_method: Option<String>,
    pub ssh_key_path: Option<String>,
}

#[derive(Deserialize)]
struct ServersResponse {
    servers: Vec<Server>,
}

pub async fn list_servers() -> Result<Vec<Server>> {
    let url = format!("{}/api/servers", server_url());
    let resp: ServersResponse = client()
        .get(&url)
        .send()
        .await
        .map_err(|e| send_err(e, &url))?
        .error_for_status()
        .with_context(|| format!("server returned an error for GET {url}"))?
        .json()
        .await
        .with_context(|| format!("failed to parse response from GET {url}"))?;
    Ok(resp.servers)
}

pub async fn get_server(id: &str) -> Result<Server> {
    let url = format!("{}/api/servers/{id}", server_url());
    let server: Server = client()
        .get(&url)
        .send()
        .await
        .map_err(|e| send_err(e, &url))?
        .error_for_status()
        .with_context(|| format!("server returned an error for GET {url}"))?
        .json()
        .await
        .with_context(|| format!("failed to parse response from GET {url}"))?;
    Ok(server)
}

pub async fn create_server(input: CreateServerInput) -> Result<serde_json::Value> {
    let url = format!("{}/api/servers", server_url());
    let resp: serde_json::Value = client()
        .post(&url)
        .json(&input)
        .send()
        .await
        .map_err(|e| send_err(e, &url))?
        .error_for_status()
        .with_context(|| format!("server returned an error for POST {url}"))?
        .json()
        .await
        .with_context(|| format!("failed to parse response from POST {url}"))?;
    Ok(resp)
}

pub async fn delete_server(id: &str) -> Result<()> {
    let url = format!("{}/api/servers/{id}", server_url());
    client()
        .delete(&url)
        .send()
        .await
        .map_err(|e| send_err(e, &url))?
        .error_for_status()
        .with_context(|| format!("server returned an error for DELETE {url}"))?;
    Ok(())
}

pub async fn get_server_status(id: &str) -> Result<serde_json::Value> {
    let url = format!("{}/api/servers/{id}/status", server_url());
    let resp: serde_json::Value = client()
        .get(&url)
        .send()
        .await
        .map_err(|e| send_err(e, &url))?
        .error_for_status()
        .with_context(|| format!("server returned an error for GET {url}"))?
        .json()
        .await
        .with_context(|| format!("failed to parse response from GET {url}"))?;
    Ok(resp)
}

// ── Tasks ─────────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct TasksResponse {
    tasks: Vec<Task>,
}

pub async fn list_tasks(server_id: Option<&str>) -> Result<Vec<Task>> {
    let base = format!("{}/api/tasks", server_url());
    let url = match server_id {
        Some(sid) => format!("{base}?server_id={sid}"),
        None => base,
    };
    let resp: TasksResponse = client()
        .get(&url)
        .send()
        .await
        .map_err(|e| send_err(e, &url))?
        .error_for_status()
        .with_context(|| format!("server returned an error for GET {url}"))?
        .json()
        .await
        .with_context(|| format!("failed to parse response from GET {url}"))?;
    Ok(resp.tasks)
}

pub async fn get_task(id: &str) -> Result<Task> {
    let url = format!("{}/api/tasks/{id}", server_url());
    let task: Task = client()
        .get(&url)
        .send()
        .await
        .map_err(|e| send_err(e, &url))?
        .error_for_status()
        .with_context(|| format!("server returned an error for GET {url}"))?
        .json()
        .await
        .with_context(|| format!("failed to parse response from GET {url}"))?;
    Ok(task)
}
