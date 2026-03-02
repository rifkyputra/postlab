use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use chrono::Utc;
use postlab_core::{
    models::{CreateServer, Server},
    modules::system,
    os_detect::OsFamily,
    ssh::{AuthMethod, SshSession},
};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::state::AppState;

pub async fn list_servers(State(state): State<AppState>) -> Json<Value> {
    match sqlx::query_as::<_, Server>("SELECT * FROM servers ORDER BY created_at DESC")
        .fetch_all(&state.pool)
        .await
    {
        Ok(servers) => Json(json!({ "servers": servers })),
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

pub async fn create_server(
    State(state): State<AppState>,
    Json(body): Json<CreateServer>,
) -> (StatusCode, Json<Value>) {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    let port = body.port.unwrap_or(22);
    let user = body.user.unwrap_or_else(|| "root".to_string());
    let auth_method = body.auth_method.unwrap_or_else(|| "key".to_string());

    let result = sqlx::query(
        "INSERT INTO servers (id, name, host, port, user, auth_method, ssh_key_path, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&body.name)
    .bind(&body.host)
    .bind(port)
    .bind(&user)
    .bind(&auth_method)
    .bind(&body.ssh_key_path)
    .bind(&now)
    .bind(&now)
    .execute(&state.pool)
    .await;

    match result {
        Ok(_) => (
            StatusCode::CREATED,
            Json(json!({ "id": id, "name": body.name })),
        ),
        Err(e) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({ "error": e.to_string() })),
        ),
    }
}

pub async fn get_server(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> (StatusCode, Json<Value>) {
    match sqlx::query_as::<_, Server>("SELECT * FROM servers WHERE id = ?")
        .bind(&id)
        .fetch_optional(&state.pool)
        .await
    {
        Ok(Some(server)) => (StatusCode::OK, Json(json!(server))),
        Ok(None) => (StatusCode::NOT_FOUND, Json(json!({ "error": "server not found" }))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        ),
    }
}

/// Connect via SSH and return live system metrics.
pub async fn server_status(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> (StatusCode, Json<Value>) {
    // 1. Load server record
    let server = match sqlx::query_as::<_, Server>("SELECT * FROM servers WHERE id = ?")
        .bind(&id)
        .fetch_optional(&state.pool)
        .await
    {
        Ok(Some(s)) => s,
        Ok(None) => {
            return (StatusCode::NOT_FOUND, Json(json!({ "error": "server not found" })))
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": e.to_string() })),
            )
        }
    };

    // 2. Build auth
    let auth = match server.auth_method.as_str() {
        "password" => {
            return (
                StatusCode::NOT_IMPLEMENTED,
                Json(json!({ "error": "password auth not supported for status — use key auth" })),
            )
        }
        _ => AuthMethod::Key(
            server
                .ssh_key_path
                .clone()
                .unwrap_or_else(|| format!("{}/.ssh/id_ed25519", std::env::var("HOME").unwrap_or_default())),
        ),
    };

    // 3. Connect and query
    let ssh = match SshSession::connect(&server.host, server.port as u16, &server.user, auth).await
    {
        Ok(s) => s,
        Err(e) => {
            return (
                StatusCode::BAD_GATEWAY,
                Json(json!({ "error": format!("SSH connect failed: {e}") })),
            )
        }
    };

    // 4. Detect OS (update record if not yet known)
    if server.os_family.is_none()
        && let Ok(os) = OsFamily::detect_remote(&ssh).await
    {
        let _ = sqlx::query("UPDATE servers SET os_family = ?, updated_at = ? WHERE id = ?")
            .bind(os.as_str())
            .bind(Utc::now().to_rfc3339())
            .bind(&id)
            .execute(&state.pool)
            .await;
    }

    // 5. Get system metrics
    let status = match system::get_status(&ssh).await {
        Ok(s) => s,
        Err(e) => {
            return (
                StatusCode::BAD_GATEWAY,
                Json(json!({ "error": format!("SSH exec failed: {e}") })),
            )
        }
    };

    let _ = ssh.disconnect().await;

    (StatusCode::OK, Json(json!(status)))
}

pub async fn delete_server(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> (StatusCode, Json<Value>) {
    match sqlx::query("DELETE FROM servers WHERE id = ?")
        .bind(&id)
        .execute(&state.pool)
        .await
    {
        Ok(r) if r.rows_affected() > 0 => {
            (StatusCode::OK, Json(json!({ "deleted": id })))
        }
        Ok(_) => (StatusCode::NOT_FOUND, Json(json!({ "error": "server not found" }))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        ),
    }
}
