use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use postlab_core::models::CreateTask;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::state::AppState;

#[derive(Deserialize)]
pub struct TaskFilter {
    pub server_id: Option<String>,
}

pub async fn list_tasks(
    State(state): State<AppState>,
    Query(filter): Query<TaskFilter>,
) -> Json<Value> {
    match state.engine.list_tasks(filter.server_id.as_deref()).await {
        Ok(tasks) => Json(json!({ "tasks": tasks })),
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

pub async fn create_task(
    State(state): State<AppState>,
    Json(body): Json<CreateTask>,
) -> (StatusCode, Json<Value>) {
    match state
        .engine
        .spawn_task(&body.server_id, &body.kind, body.input_json)
        .await
    {
        Ok(id) => (StatusCode::CREATED, Json(json!({ "id": id }))),
        Err(e) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({ "error": e.to_string() })),
        ),
    }
}

pub async fn get_task(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> (StatusCode, Json<Value>) {
    match state.engine.get_task(&id).await {
        Ok(Some(task)) => (StatusCode::OK, Json(json!(task))),
        Ok(None) => (StatusCode::NOT_FOUND, Json(json!({ "error": "task not found" }))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        ),
    }
}
