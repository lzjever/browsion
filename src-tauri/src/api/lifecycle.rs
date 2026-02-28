//! Browser lifecycle HTTP handlers: launch, kill, running.

use super::{ApiResult, ApiState};
use crate::commands::get_effective_chrome_path_from_config;
use axum::{
    extract::{Path as AxumPath, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};

pub fn router() -> Router<ApiState> {
    Router::new()
        .route("/api/launch/:profile_id", post(launch_profile))
        .route("/api/kill/:profile_id", post(kill_profile))
        .route("/api/running", get(get_running_browsers))
}

#[derive(serde::Serialize)]
pub struct LaunchResponse {
    pub pid: u32,
    pub cdp_port: u16,
}

#[derive(serde::Serialize)]
struct RunningBrowser {
    profile_id: String,
    pid: u32,
    cdp_port: Option<u16>,
    launched_at: u64,
}

async fn launch_profile(
    State(state): State<ApiState>,
    AxumPath(profile_id): AxumPath<String>,
) -> ApiResult<Json<LaunchResponse>> {
    let config = state.config.read().clone();
    let _profile = config
        .profiles
        .iter()
        .find(|p| p.id == profile_id)
        .ok_or_else(|| (StatusCode::NOT_FOUND, "Profile not found".to_string()))?;
    if state.process_manager.is_running(&profile_id) {
        return Err((
            StatusCode::CONFLICT,
            "Profile is already running".to_string(),
        ));
    }
    let chrome_path = get_effective_chrome_path_from_config(&config)
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    let (pid, cdp_port) = state
        .process_manager
        .launch_profile(&profile_id, &config, &chrome_path)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    {
        let mut config = state.config.write();
        config.recent_profiles.retain(|id| id != &profile_id);
        config.recent_profiles.insert(0, profile_id.clone());
        if config.recent_profiles.len() > 10 {
            config.recent_profiles.truncate(10);
        }
        if let Err(e) = crate::config::save_config(&config) {
            tracing::warn!("Failed to save recent profiles after launch: {}", e);
        }
    }
    // Persist session for reconnect across Tauri restarts
    let pid_id = profile_id.clone();
    tokio::spawn(async move {
        if let Err(e) = crate::process::sessions_persist::save_session(&pid_id, pid, cdp_port).await {
            tracing::warn!("Failed to persist session for {}: {}", pid_id, e);
        }
    });
    state.emit("browser-status-changed");
    Ok(Json(LaunchResponse { pid, cdp_port }))
}

async fn kill_profile(
    State(state): State<ApiState>,
    AxumPath(profile_id): AxumPath<String>,
) -> ApiResult<StatusCode> {
    state.session_manager.disconnect(&profile_id).await;
    state
        .process_manager
        .kill_profile(&profile_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    // Remove from persisted sessions
    let kill_id = profile_id.clone();
    tokio::spawn(async move {
        if let Err(e) = crate::process::sessions_persist::remove_session(&kill_id).await {
            tracing::warn!("Failed to remove persisted session for {}: {}", kill_id, e);
        }
    });
    state.emit("browser-status-changed");
    Ok(StatusCode::NO_CONTENT)
}

async fn get_running_browsers(State(state): State<ApiState>) -> Json<Vec<RunningBrowser>> {
    let ids = state.process_manager.get_running_profiles();
    let browsers: Vec<RunningBrowser> = ids
        .iter()
        .filter_map(|id| {
            state
                .process_manager
                .get_process_info(id)
                .map(|info| RunningBrowser {
                    profile_id: info.profile_id,
                    pid: info.pid,
                    cdp_port: info.cdp_port,
                    launched_at: info.launched_at,
                })
        })
        .collect();
    Json(browsers)
}
