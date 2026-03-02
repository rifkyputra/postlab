use axum::{extract::State, Json};
use postlab_core::models::AuditLog;
use serde_json::{json, Value};

use crate::state::AppState;

pub async fn list_audit(State(state): State<AppState>) -> Json<Value> {
    match sqlx::query_as::<_, AuditLog>("SELECT * FROM audit_log ORDER BY created_at DESC")
        .fetch_all(&state.pool)
        .await
    {
        Ok(entries) => Json(json!({ "audit": entries })),
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}
