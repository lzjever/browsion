//! Profile CRUD HTTP handlers.

use super::{ApiResult, ApiState};
use crate::config::{validation, BrowserProfile};
use axum::{
    extract::{Path as AxumPath, State},
    http::StatusCode,
    routing::{delete, get, post, put},
    Json, Router,
};

pub fn router() -> Router<ApiState> {
    Router::new()
        .route("/api/profiles", get(list_profiles).post(add_profile))
        .route(
            "/api/profiles/:id",
            get(get_profile).put(update_profile).delete(delete_profile),
        )
}

async fn list_profiles(
    State(state): State<ApiState>,
) -> Result<Json<Vec<serde_json::Value>>, (StatusCode, String)> {
    let config = state.config.read();
    let profiles: Vec<serde_json::Value> = config
        .profiles
        .iter()
        .map(|p| {
            let mut v = serde_json::to_value(p).unwrap_or_default();
            if let Some(obj) = v.as_object_mut() {
                obj.insert(
                    "is_running".to_string(),
                    serde_json::Value::Bool(state.process_manager.is_running(&p.id)),
                );
            }
            v
        })
        .collect();
    Ok(Json(profiles))
}

async fn get_profile(
    State(state): State<ApiState>,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<BrowserProfile>, (StatusCode, String)> {
    let config = state.config.read();
    let profile = config
        .profiles
        .iter()
        .find(|p| p.id == id)
        .cloned()
        .ok_or_else(|| (StatusCode::NOT_FOUND, "Profile not found".to_string()))?;
    Ok(Json(profile))
}

async fn add_profile(
    State(state): State<ApiState>,
    Json(profile): Json<BrowserProfile>,
) -> Result<(StatusCode, Json<BrowserProfile>), (StatusCode, String)> {
    validation::validate_profile(&profile).map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    let mut config = state.config.write();
    if config.profiles.iter().any(|p| p.id == profile.id) {
        return Err((
            StatusCode::CONFLICT,
            "Profile ID already exists".to_string(),
        ));
    }
    config.profiles.push(profile.clone());
    crate::config::save_config(&config)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    drop(config);
    state.emit("profiles-changed");
    Ok((StatusCode::CREATED, Json(profile)))
}

async fn update_profile(
    State(state): State<ApiState>,
    AxumPath(id): AxumPath<String>,
    Json(profile): Json<BrowserProfile>,
) -> Result<Json<BrowserProfile>, (StatusCode, String)> {
    if profile.id != id {
        return Err((
            StatusCode::BAD_REQUEST,
            "ID in path and body must match".to_string(),
        ));
    }
    validation::validate_profile(&profile).map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    let mut config = state.config.write();
    let pos = config
        .profiles
        .iter()
        .position(|p| p.id == id)
        .ok_or_else(|| (StatusCode::NOT_FOUND, "Profile not found".to_string()))?;
    config.profiles[pos] = profile.clone();
    crate::config::save_config(&config)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    drop(config);
    state.emit("profiles-changed");
    Ok(Json(profile))
}

async fn delete_profile(
    State(state): State<ApiState>,
    AxumPath(id): AxumPath<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    if state.process_manager.is_running(&id) {
        return Err((
            StatusCode::CONFLICT,
            "Cannot delete profile while it is running".to_string(),
        ));
    }
    let mut config = state.config.write();
    let before = config.profiles.len();
    config.profiles.retain(|p| p.id != id);
    if config.profiles.len() == before {
        return Err((StatusCode::NOT_FOUND, "Profile not found".to_string()));
    }
    crate::config::save_config(&config)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    drop(config);
    state.emit("profiles-changed");
    Ok(StatusCode::NO_CONTENT)
}
