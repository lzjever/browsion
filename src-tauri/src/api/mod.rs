//! Local HTTP API for profile management and browser lifecycle.
//!
//! Profile CRUD, browser launch/kill, settings, snapshots, WebSocket, health check.

pub mod lifecycle;
pub mod ws;

use crate::config::{validation, BrowserProfile};
use crate::state::AppState;
use axum::{
    extract::{Path as AxumPath, State},
    http::StatusCode,
    routing::{delete, get, post},
    Json, Router,
};
use std::sync::Arc;

pub type ApiState = Arc<AppState>;
pub type ApiResult<T> = Result<T, (StatusCode, String)>;

// ---------------------------------------------------------------------------
// Profile CRUD routes
// ---------------------------------------------------------------------------

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

#[derive(serde::Deserialize)]
struct CreateProfileReq {
    profile: BrowserProfile,
}

async fn add_profile(
    State(state): State<ApiState>,
    Json(req): Json<CreateProfileReq>,
) -> ApiResult<Json<BrowserProfile>> {
    validation::validate_profile(&req.profile).map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

    let mut config = state.config.write();
    config.profiles.push(req.profile.clone());
    crate::config::save_config(&config).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    drop(config);
    state.emit("profiles-changed");
    Ok(Json(req.profile))
}

async fn get_profile(
    State(state): State<ApiState>,
    AxumPath(id): AxumPath<String>,
) -> ApiResult<Json<BrowserProfile>> {
    let config = state.config.read();
    let profile = config
        .profiles
        .iter()
        .find(|p| p.id == id)
        .cloned()
        .ok_or((StatusCode::NOT_FOUND, "Profile not found".to_string()))?;
    Ok(Json(profile))
}

async fn update_profile(
    State(state): State<ApiState>,
    AxumPath(id): AxumPath<String>,
    Json(profile): Json<BrowserProfile>,
) -> ApiResult<Json<BrowserProfile>> {
    if profile.id != id {
        return Err((
            StatusCode::BAD_REQUEST,
            "ID in path and body must match".to_string(),
        ));
    }
    validation::validate_profile(&profile).map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

    let mut config = state.config.write();
    if let Some(pos) = config.profiles.iter().position(|p| p.id == id) {
        config.profiles[pos] = profile.clone();
        crate::config::save_config(&config)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        drop(config);
        state.emit("profiles-changed");
        Ok(Json(profile))
    } else {
        Err((StatusCode::NOT_FOUND, "Profile not found".to_string()))
    }
}

async fn delete_profile(
    State(state): State<ApiState>,
    AxumPath(id): AxumPath<String>,
) -> ApiResult<StatusCode> {
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

// ---------------------------------------------------------------------------
// Browser lifecycle routes
// ---------------------------------------------------------------------------

#[derive(serde::Deserialize)]
struct RegisterExternalReq {
    profile_id: String,
    pid: u32,
    cdp_port: u16,
}

async fn register_external_profile(
    State(state): State<ApiState>,
    Json(req): Json<RegisterExternalReq>,
) -> ApiResult<Json<serde_json::Value>> {
    if state.process_manager.is_running(&req.profile_id) {
        return Err((
            StatusCode::CONFLICT,
            "Profile is already running".to_string(),
        ));
    }

    state
        .process_manager
        .register_external(&req.profile_id, req.pid, req.cdp_port);

    // Persist session for reconnect
    let profile_id = req.profile_id.clone();
    let pid = req.pid;
    let cdp_port = req.cdp_port;
    tokio::spawn(async move {
        if let Err(e) = crate::process::sessions_persist::save_session(&profile_id, pid, cdp_port).await {
            tracing::warn!("Failed to persist session for {}: {}", profile_id, e);
        }
    });

    state.emit("browser-status-changed");
    Ok(Json(serde_json::json!({ "ok": true, "profile_id": req.profile_id })))
}

// ---------------------------------------------------------------------------
// Settings routes
// ---------------------------------------------------------------------------

async fn get_app_settings(
    State(state): State<ApiState>,
) -> Json<crate::config::AppSettings> {
    let config = state.config.read();
    Json(config.settings.clone())
}

#[derive(serde::Deserialize)]
struct UpdateAppSettingsReq {
    settings: crate::config::AppSettings,
}

async fn update_app_settings(
    State(state): State<ApiState>,
    Json(req): Json<UpdateAppSettingsReq>,
) -> ApiResult<Json<crate::config::AppSettings>> {
    let mut config = state.config.write();
    config.settings = req.settings.clone();
    crate::config::save_config(&config).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(config.settings.clone()))
}

// ---------------------------------------------------------------------------
// Browser source routes
// ---------------------------------------------------------------------------

async fn get_browser_source(
    State(state): State<ApiState>,
) -> Json<crate::config::schema::BrowserSource> {
    let config = state.config.read();
    Json(config.browser_source.clone())
}

#[derive(serde::Deserialize)]
struct UpdateBrowserSourceReq {
    browser_source: crate::config::schema::BrowserSource,
}

async fn update_browser_source(
    State(state): State<ApiState>,
    Json(req): Json<UpdateBrowserSourceReq>,
) -> ApiResult<Json<crate::config::schema::BrowserSource>> {
    let mut config = state.config.write();
    config.browser_source = req.browser_source.clone();
    crate::config::save_config(&config).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(config.browser_source.clone()))
}

// ---------------------------------------------------------------------------
// Local API config routes
// ---------------------------------------------------------------------------

async fn get_local_api_config(
    State(state): State<ApiState>,
) -> Json<crate::config::schema::McpConfig> {
    let config = state.config.read();
    Json(config.mcp.clone())
}

#[derive(serde::Deserialize)]
struct UpdateLocalApiConfigReq {
    mcp: crate::config::schema::McpConfig,
}

async fn update_local_api_config(
    State(state): State<ApiState>,
    Json(req): Json<UpdateLocalApiConfigReq>,
) -> ApiResult<Json<crate::config::schema::McpConfig>> {
    {
        let mut config = state.config.write();
        config.mcp = req.mcp.clone();
        crate::config::save_config(&config).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    }

    // Stop the existing server
    {
        let mut guard = state.api_server_abort.lock();
        if let Some(abort_fn) = guard.take() {
            abort_fn();
            tracing::info!("Stopped API server for reconfiguration");
        }
    }

    // Brief pause to let the old listener release the port
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Restart if enabled
    if req.mcp.enabled && req.mcp.api_port > 0 {
        let state_clone = Arc::clone(&state);
        let api_key = req.mcp.api_key.clone();
        let port = req.mcp.api_port;
        let handle = tokio::spawn(async move {
            if let Err(e) = crate::api::run_server(state_clone, port, api_key).await {
                tracing::error!("API server error after restart: {}", e);
            }
        });
        let mut guard = state.api_server_abort.lock();
        *guard = Some(Box::new(move || handle.abort()));
        tracing::info!("Restarted API server on port {}", req.mcp.api_port);
    }

    let config = state.config.read();
    Ok(Json(config.mcp.clone()))
}

// ---------------------------------------------------------------------------
// Snapshots routes
// ---------------------------------------------------------------------------

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
    let info = crate::commands::snapshots::core_create_snapshot(&id, &req.name, &config, &state.process_manager)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(info))
}

async fn restore_snapshot(
    State(state): State<ApiState>,
    AxumPath((id, name)): AxumPath<(String, String)>,
) -> ApiResult<Json<serde_json::Value>> {
    let config = state.config.read().clone();
    crate::commands::snapshots::core_restore_snapshot(&id, &name, &config, &state.process_manager)
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

// ---------------------------------------------------------------------------
// Health check
// ---------------------------------------------------------------------------

async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "ok": true }))
}

// ---------------------------------------------------------------------------
// Build the main API router
// ---------------------------------------------------------------------------

pub fn router(state: ApiState) -> Router {
    Router::new()
        // Profile routes
        .route("/api/profiles", get(list_profiles).post(add_profile))
        .route(
            "/api/profiles/:id",
            get(get_profile).put(update_profile).delete(delete_profile),
        )
        // Lifecycle routes
        .route("/api/launch/:profile_id", post(crate::api::lifecycle::launch_profile))
        .route("/api/kill/:profile_id", post(crate::api::lifecycle::kill_profile))
        .route("/api/register-external", post(register_external_profile))
        .route("/api/running", get(crate::api::lifecycle::get_running_browsers))
        // Snapshots routes
        .route("/api/profiles/:id/snapshots", get(list_snapshots).post(create_snapshot))
        .route("/api/profiles/:id/snapshots/:name/restore", post(restore_snapshot))
        .route("/api/profiles/:id/snapshots/:name", delete(delete_snapshot))
        // Settings routes
        .route("/api/settings", get(get_app_settings).put(update_app_settings))
        // Browser source routes
        .route("/api/browser-source", get(get_browser_source).put(update_browser_source))
        // Local API config routes
        .route("/api/local-api", get(get_local_api_config).put(update_local_api_config))
        // WebSocket + health
        .route("/api/ws", axum::routing::get(ws::ws_handler))
        .route("/api/health", axum::routing::get(health))
        .with_state(state)
}

// ---------------------------------------------------------------------------
// Server
// ---------------------------------------------------------------------------

/// Build the full API app (router + CORS).
pub fn app(state: ApiState, _api_key: Option<String>) -> Router {
    use tower::limit::ConcurrencyLimitLayer;
    let base_router = router(state);
    base_router
        .layer(ConcurrencyLimitLayer::new(32))
        .layer(
            tower_http::cors::CorsLayer::new()
                .allow_origin(tower_http::cors::Any)
                .allow_methods([
                    axum::http::Method::GET,
                    axum::http::Method::POST,
                    axum::http::Method::PUT,
                    axum::http::Method::DELETE,
                ])
                .allow_headers([
                    axum::http::header::CONTENT_TYPE,
                    axum::http::HeaderName::from_static("x-api-key"),
                ]),
        )
}

pub async fn run_server(state: ApiState, port: u16, api_key: Option<String>) -> Result<(), String> {
    let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{}", port))
        .await
        .map_err(|e| format!("Failed to bind API port {}: {}", port, e))?;
    let app = app(state, api_key);
    tracing::info!("Browsion API listening on http://127.0.0.1:{}", port);
    axum::serve(listener, app)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}
