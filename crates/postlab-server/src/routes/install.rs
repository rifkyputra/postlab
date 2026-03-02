use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::state::AppState;

#[derive(Deserialize)]
pub struct InstallBody {
    pub app: String,
}

pub async fn install_app(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<InstallBody>,
) -> (StatusCode, Json<Value>) {
    let input = json!({ "app": body.app });
    match state.engine.spawn_task(&id, "install_app", Some(input)).await {
        Ok(task_id) => (StatusCode::ACCEPTED, Json(json!({ "task_id": task_id }))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        ),
    }
}

pub async fn upgrade_os(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> (StatusCode, Json<Value>) {
    match state.engine.spawn_task(&id, "upgrade_os", None).await {
        Ok(task_id) => (StatusCode::ACCEPTED, Json(json!({ "task_id": task_id }))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        ),
    }
}

pub async fn harden(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> (StatusCode, Json<Value>) {
    match state.engine.spawn_task(&id, "harden", None).await {
        Ok(task_id) => (StatusCode::ACCEPTED, Json(json!({ "task_id": task_id }))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        ),
    }
}
