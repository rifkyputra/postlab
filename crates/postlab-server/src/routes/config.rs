use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use chrono::Utc;
use postlab_core::models::Config;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::state::AppState;

pub async fn list_config(State(state): State<AppState>) -> Json<Value> {
    match sqlx::query_as::<_, Config>("SELECT * FROM config")
        .fetch_all(&state.pool)
        .await
    {
        Ok(entries) => Json(json!({ "config": entries })),
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

#[derive(Deserialize)]
pub struct SetConfig {
    pub value: String,
}

pub async fn set_config(
    State(state): State<AppState>,
    Path(key): Path<String>,
    Json(body): Json<SetConfig>,
) -> (StatusCode, Json<Value>) {
    let now = Utc::now().to_rfc3339();
    let result = sqlx::query(
        "INSERT INTO config (key, value, updated_at) VALUES (?, ?, ?)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
    )
    .bind(&key)
    .bind(&body.value)
    .bind(&now)
    .execute(&state.pool)
    .await;

    match result {
        Ok(_) => (StatusCode::OK, Json(json!({ "key": key, "value": body.value }))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        ),
    }
}
