//! Moderation (bearer `VITRINE_API_ADMIN_TOKEN`).

use std::sync::Arc;

use axum::extract::Path;
use axum::http::HeaderMap;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{Value, json};

use super::St;
use crate::AppState;
use crate::auth;
use crate::db::{self, APPROVED, PENDING, REJECTED};
use crate::error::{ApiError, ApiResult};

pub(super) fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/v1/admin/queue", get(queue))
        .route("/v1/admin/templates/{id}/{action}", post(action))
}

async fn queue(axum::extract::State(state): St, headers: HeaderMap) -> ApiResult<Json<Value>> {
    auth::admin(&state, &headers)?;
    let db = state.db();
    let pending = db::search(&db, PENDING, None, None, 100, 0)?;
    let reports = db::open_reports(&db)?;
    Ok(Json(json!({ "pending": pending, "reports": reports })))
}

#[derive(Debug, Default, Deserialize)]
struct ModerationBody {
    #[serde(default)]
    note: String,
}

async fn action(
    axum::extract::State(state): St,
    headers: HeaderMap,
    Path((id, action)): Path<(String, String)>,
    body: Option<Json<ModerationBody>>,
) -> ApiResult<Json<Value>> {
    auth::admin(&state, &headers)?;
    let status = match action.as_str() {
        "approve" => APPROVED,
        "reject" => REJECTED,
        _ => return Err(ApiError::not_found("action")),
    };
    let note = body.map(|b| b.0.note).unwrap_or_default();
    let db = state.db();
    if !db::set_status(&db, &id, status, &note)? {
        return Err(ApiError::not_found("template"));
    }
    db::resolve_reports(&db, &id)?;
    Ok(Json(json!({ "id": id, "status": status })))
}
