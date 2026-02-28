//! Snapshot HTTP handlers for /api/profiles/:id/snapshots routes.

use super::{ApiResult, ApiState};
use axum::{
    extract::{Path as AxumPath, State},
    http::StatusCode,
    routing::{delete, get, post},
    Json, Router,
};

pub fn router() -> Router<ApiState> {
    Router::new()
        .route("/api/profiles/:id/snapshots", get(list_snapshots).post(create_snapshot))
        .route("/api/profiles/:id/snapshots/:name/restore", post(restore_snapshot))
        .route("/api/profiles/:id/snapshots/:name", delete(delete_snapshot))
}

async fn list_snapshots(
    State(state): State<ApiState>,
    AxumPath(id): AxumPath<String>,
) -> ApiResult<Json<Vec<crate::config::schema::SnapshotInfo>>> {
    let config = state.config.read().clone();
    let infos = crate::commands::snapshots::core_list_snapshots(&id, &config)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(infos))
}

#[derive(serde::Deserialize)]
struct CreateSnapshotReq {
    name: String,
}

async fn create_snapshot(
    State(state): State<ApiState>,
    AxumPath(id): AxumPath<String>,
    Json(req): Json<CreateSnapshotReq>,
) -> ApiResult<Json<crate::config::schema::SnapshotInfo>> {
    let config = state.config.read().clone();
    let info = crate::commands::snapshots::core_create_snapshot(
        &id,
        &req.name,
        &config,
        &state.process_manager,
    )
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(info))
}

async fn restore_snapshot(
    State(state): State<ApiState>,
    AxumPath((id, name)): AxumPath<(String, String)>,
) -> ApiResult<Json<serde_json::Value>> {
    let config = state.config.read().clone();
    crate::commands::snapshots::core_restore_snapshot(
        &id,
        &name,
        &config,
        &state.process_manager,
        &state.session_manager,
    )
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

async fn delete_snapshot(
    State(_state): State<ApiState>,
    AxumPath((id, name)): AxumPath<(String, String)>,
) -> ApiResult<StatusCode> {
    crate::commands::snapshots::core_delete_snapshot(&id, &name)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(StatusCode::NO_CONTENT)
}
