//! Local HTTP API for browser automation and app control.
//! Profile CRUD, browser launch/kill, recording, and CDP browser control.

pub mod action_log;
pub mod ws;

use crate::commands::get_effective_chrome_path_from_config;
use crate::commands::recording::{map_manual_event_to_action, normalize_recording_actions};
use crate::config::{validation, BrowserProfile};
use crate::state::AppState;
#[allow(unused_imports)]
use axum::{
    extract::{Path as AxumPath, Query, State, Request},
    http::StatusCode,
    middleware::{self, Next},
    response::IntoResponse,
    response::Response,
    routing::{delete, get, post, put},
    Json, Router,
};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

pub type ApiState = Arc<AppState>;

/// API key authentication middleware.
/// Skips authentication for GET /api/health so local tooling can probe the server.
async fn api_key_auth(
    axum::extract::State(expected_key): axum::extract::State<String>,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let path = request.uri().path();
    if path == "/api/health" {
        return Ok(next.run(request).await);
    }

    let header_key = request
        .headers()
        .get("X-API-Key")
        .and_then(|v| v.to_str().ok());

    let query_key = if path == "/api/ws" {
        request
            .uri()
            .query()
            .and_then(|query| {
                query.split('&').find_map(|part| {
                    let mut pieces = part.splitn(2, '=');
                    match (pieces.next(), pieces.next()) {
                        (Some("api_key"), Some(value)) => Some(value),
                        _ => None,
                    }
                })
            })
    } else {
        None
    };

    match header_key.or(query_key) {
        Some(k) if k == expected_key => Ok(next.run(request).await),
        _ => Err(StatusCode::UNAUTHORIZED),
    }
}

/// Action log middleware — records every API call with timing and outcome.
async fn action_log_middleware(
    State(state): State<ApiState>,
    request: Request,
    next: Next,
) -> Response {
    let path = request.uri().path().to_string();

    // Skip health + action_log routes themselves to avoid noise
    if path == "/api/health" || path.starts_with("/api/action_log") {
        return next.run(request).await;
    }

    // Extract profile_id and tool from path patterns:
    //   /api/browser/:id/<tool>     → profile_id = id, tool = tool_name
    //   /api/launch/:profile_id     → profile_id, tool = "launch"
    //   /api/kill/:profile_id       → profile_id, tool = "kill"
    let (profile_id, tool) = parse_path_for_log(&path);

    let t0 = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();

    let response = next.run(request).await;

    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .saturating_sub(t0);
    let duration_ms = elapsed.as_millis() as u64;
    let ts = t0.as_millis() as u64;

    let status = response.status();
    let success = status.is_success() || status.is_redirection();
    let error = if !success {
        Some(format!("HTTP {}", status.as_u16()))
    } else {
        None
    };

    let entry = action_log::ActionEntry {
        id: uuid::Uuid::new_v4().to_string(),
        ts,
        profile_id: profile_id.clone(),
        tool: tool.clone(),
        duration_ms,
        success,
        error,
    };

    let log = state.action_log.clone();
    let entry_clone = entry.clone();
    tokio::spawn(async move {
        log.push(entry_clone.clone());
        action_log::append_to_file(&entry_clone).await;
    });

    // Broadcast to WebSocket clients
    state.broadcast_ws(ws::WsEvent::ActionLogEntry {
        id: entry.id.clone(),
        ts: entry.ts,
        profile_id: entry.profile_id.clone(),
        tool: entry.tool.clone(),
        duration_ms: entry.duration_ms,
        success: entry.success,
        error: entry.error.clone(),
    });

    response
}

fn parse_path_for_log(path: &str) -> (String, String) {
    let parts: Vec<&str> = path.trim_start_matches('/').split('/').collect();
    // /api/browser/:id/<tool>
    if parts.len() >= 4 && parts[0] == "api" && parts[1] == "browser" {
        let profile_id = parts[2].to_string();
        let tool = parts[3..].join("/");
        return (profile_id, tool);
    }
    // /api/launch/:id or /api/kill/:id
    if parts.len() >= 3 && parts[0] == "api" && (parts[1] == "launch" || parts[1] == "kill") {
        return (parts[2].to_string(), parts[1].to_string());
    }
    // /api/profiles/:id (CRUD)
    if parts.len() >= 3 && parts[0] == "api" && parts[1] == "profiles" {
        return (String::new(), format!("profiles/{}", parts[2]));
    }
    // fallback
    (String::new(), parts[1..].join("/"))
}

#[cfg(test)]
fn tool_to_recorded_action(tool: &str) -> Option<crate::recording::RecordedActionType> {
    use crate::recording::RecordedActionType;

    match tool {
        "navigate" | "navigate_wait" => Some(RecordedActionType::Navigate),
        "click" => Some(RecordedActionType::Click),
        "hover" => Some(RecordedActionType::Hover),
        "double_click" => Some(RecordedActionType::DoubleClick),
        "right_click" => Some(RecordedActionType::RightClick),
        "type" => Some(RecordedActionType::Type),
        "slow_type" => Some(RecordedActionType::SlowType),
        "press_key" => Some(RecordedActionType::PressKey),
        "select_option" => Some(RecordedActionType::SelectOption),
        "upload_file" => Some(RecordedActionType::UploadFile),
        "scroll" => Some(RecordedActionType::Scroll),
        "scroll_into_view" => Some(RecordedActionType::ScrollIntoView),
        "new_tab" | "tabs/new" => Some(RecordedActionType::NewTab),
        "tabs/switch" => Some(RecordedActionType::SwitchTab),
        "tabs/close" => Some(RecordedActionType::CloseTab),
        "wait_for_text" => Some(RecordedActionType::WaitForText),
        "wait_for" => Some(RecordedActionType::WaitForElement),
        "screenshot" => Some(RecordedActionType::Screenshot),
        "console" => Some(RecordedActionType::GetConsoleLogs),
        "extract" => Some(RecordedActionType::Extract),
        _ => None,
    }
}

fn record_browser_action(
    state: &ApiState,
    profile_id: &str,
    action_type: crate::recording::RecordedActionType,
    params: serde_json::Value,
) {
    let _ = state
        .recording_session_manager
        .add_action(profile_id, action_type, params);
}

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

pub fn router(state: ApiState) -> Router {
    Router::new()
        // Profile CRUD
        .route("/api/profiles", get(list_profiles).post(add_profile))
        .route(
            "/api/profiles/:id",
            get(get_profile).put(update_profile).delete(delete_profile),
        )
        // Browser lifecycle
        .route("/api/launch/:profile_id", post(launch_profile))
        .route("/api/kill/:profile_id", post(kill_profile))
        .route("/api/register-external", post(register_external_profile))
        .route("/api/running", get(get_running_browsers))
        // Browser control (CDP)
        .route("/api/browser/:id/navigate", post(browser_navigate))
        .route("/api/browser/:id/navigate_wait", post(browser_navigate_wait))
        .route("/api/browser/:id/url", get(browser_get_url))
        .route("/api/browser/:id/title", get(browser_get_title))
        .route("/api/browser/:id/back", post(browser_go_back))
        .route("/api/browser/:id/forward", post(browser_go_forward))
        .route("/api/browser/:id/reload", post(browser_reload))
        .route("/api/browser/:id/click", post(browser_click))
        .route("/api/browser/:id/hover", post(browser_hover))
        .route("/api/browser/:id/double_click", post(browser_double_click))
        .route("/api/browser/:id/right_click", post(browser_right_click))
        .route("/api/browser/:id/type", post(browser_type_text))
        .route("/api/browser/:id/slow_type", post(browser_slow_type))
        .route("/api/browser/:id/press_key", post(browser_press_key))
        .route("/api/browser/:id/scroll", post(browser_scroll))
        .route("/api/browser/:id/scroll_into_view", post(browser_scroll_into_view))
        .route("/api/browser/:id/select_option", post(browser_select_option))
        .route("/api/browser/:id/wait_for", post(browser_wait_for))
        .route("/api/browser/:id/wait_for_nav", post(browser_wait_for_navigation))
        .route("/api/browser/:id/upload_file", post(browser_upload_file))
        .route("/api/browser/:id/screenshot", get(browser_screenshot))
        .route("/api/browser/:id/screenshot_element", get(browser_screenshot_element))
        .route("/api/browser/:id/dom_context", get(browser_dom_context))
        .route("/api/browser/:id/ax_tree", get(browser_ax_tree))
        .route("/api/browser/:id/page_state", get(browser_page_state))
        .route("/api/browser/:id/click_ref", post(browser_click_ref))
        .route("/api/browser/:id/type_ref", post(browser_type_ref))
        .route("/api/browser/:id/focus_ref", post(browser_focus_ref))
        .route("/api/browser/:id/extract", post(browser_extract))
        .route("/api/browser/:id/evaluate", post(browser_evaluate))
        // Advanced: Tabs
        .route("/api/browser/:id/tabs", get(browser_list_tabs))
        .route("/api/browser/:id/tabs/new", post(browser_new_tab))
        .route("/api/browser/:id/tabs/switch", post(browser_switch_tab))
        .route("/api/browser/:id/tabs/close", post(browser_close_tab))
        .route("/api/browser/:id/tabs/wait_new", post(browser_wait_for_new_tab))
        // Advanced: Cookies
        .route("/api/browser/:id/cookies", get(browser_get_cookies))
        .route("/api/browser/:id/cookies/set", post(browser_set_cookie))
        .route("/api/browser/:id/cookies/clear", post(browser_delete_cookies))
        // Advanced: Console
        .route("/api/browser/:id/console", get(browser_get_console_logs))
        .route("/api/browser/:id/console/enable", post(browser_enable_console))
        // Advanced: dialog, coordinate click, drag, network log, wait_for_text
        .route("/api/browser/:id/handle_dialog", post(browser_handle_dialog))
        .route("/api/browser/:id/click_at", post(browser_click_at))
        .route("/api/browser/:id/drag", post(browser_drag))
        .route("/api/browser/:id/network_log", get(browser_network_log))
        .route("/api/browser/:id/network_log/clear", post(browser_clear_network_log))
        .route("/api/browser/:id/wait_for_text", post(browser_wait_for_text))
        .route("/api/browser/:id/emulate", post(browser_emulate))
        .route("/api/browser/:id/scroll_element", post(browser_scroll_element))
        .route("/api/browser/:id/wait_for_url", post(browser_wait_for_url))
        // Advanced: Storage (localStorage / sessionStorage)
        .route("/api/browser/:id/storage", get(browser_get_storage).post(browser_set_storage).delete(browser_clear_storage))
        // Advanced: Page text, Network intercept, PDF, Touch, Frames
        .route("/api/browser/:id/page_text", get(browser_get_page_text))
        .route("/api/browser/:id/intercept/block", post(browser_intercept_block))
        .route("/api/browser/:id/intercept/mock", post(browser_intercept_mock))
        .route("/api/browser/:id/intercept", delete(browser_clear_intercepts))
        .route("/api/browser/:id/pdf", get(browser_print_to_pdf))
        .route("/api/browser/:id/tap", post(browser_tap))
        .route("/api/browser/:id/swipe", post(browser_swipe))
        .route("/api/browser/:id/frames", get(browser_get_frames))
        .route("/api/browser/:id/switch_frame", post(browser_switch_frame))
        .route("/api/browser/:id/main_frame", post(browser_main_frame))
        .route("/api/browser/:id/console/clear", post(browser_clear_console))
        // Action log
        .route("/api/action_log", get(get_action_log).delete(clear_action_log))
        // Profile snapshots
        .route("/api/profiles/:id/snapshots", get(list_snapshots).post(create_snapshot))
        .route("/api/profiles/:id/snapshots/:name/restore", post(restore_snapshot))
        .route("/api/profiles/:id/snapshots/:name", delete(delete_snapshot))
        // Cookie export/import
        .route("/api/browser/:id/cookies/export", get(browser_export_cookies))
        .route("/api/browser/:id/cookies/import", post(browser_import_cookies))
        // App control
        .route("/api/settings", get(get_app_settings).put(update_app_settings))
        .route("/api/browser-source", get(get_browser_source).put(update_browser_source))
        .route("/api/local-api", get(get_local_api_config).put(update_local_api_config))
        // Recordings
        .route("/api/recordings", get(list_saved_recordings).post(save_recording))
        .route("/api/recordings/:id", get(get_saved_recording).delete(delete_saved_recording))
        .route("/api/recordings/:id/play/:profile_id", post(play_recording))
        .route("/api/recordings/start/:profile_id", post(start_recording))
        .route("/api/recordings/stop/:session_id", post(stop_recording))
        .route("/api/profiles/:id/recording-status", get(get_recording_status))
        // WebSocket (real-time events)
        .route("/api/ws", axum::routing::get(ws::ws_handler))
        // Utility
        .route("/api/health", get(health))
        .with_state(state)
}

// ---------------------------------------------------------------------------
// Helper: resolve CDP port for a running profile
// ---------------------------------------------------------------------------

fn require_cdp_port(state: &AppState, profile_id: &str) -> Result<u16, ApiError> {
    state
        .process_manager
        .get_cdp_port(profile_id)
        .ok_or_else(|| ApiError::new(
            StatusCode::CONFLICT,
            "browser_not_running",
            format!("Profile {} is not running", profile_id),
        ).with_detail("profile_id", serde_json::json!(profile_id)))
}

// ---------------------------------------------------------------------------
// Health
// ---------------------------------------------------------------------------

async fn health() -> &'static str {
    "ok"
}

// ---------------------------------------------------------------------------
// App control
// ---------------------------------------------------------------------------

async fn get_app_settings(
    State(state): State<ApiState>,
) -> ApiResult<Json<serde_json::Value>> {
    let config = state.config.read();
    Ok(api_ok(serde_json::to_value(config.settings.clone()).unwrap_or_default()))
}

async fn update_app_settings(
    State(state): State<ApiState>,
    Json(settings): Json<crate::config::AppSettings>,
) -> ApiResult<Json<serde_json::Value>> {
    let mut config = state.config.write();
    config.settings = settings.clone();
    crate::config::save_config(&config)
        .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "config_save_failed", e.to_string()))?;
    Ok(api_ok(serde_json::to_value(settings).unwrap_or_default()))
}

async fn get_browser_source(
    State(state): State<ApiState>,
) -> ApiResult<Json<serde_json::Value>> {
    let config = state.config.read();
    Ok(api_ok(serde_json::to_value(config.browser_source.clone()).unwrap_or_default()))
}

async fn update_browser_source(
    State(state): State<ApiState>,
    Json(source): Json<crate::config::schema::BrowserSource>,
) -> ApiResult<Json<serde_json::Value>> {
    let mut config = state.config.write();
    config.browser_source = source.clone();
    crate::config::save_config(&config)
        .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "config_save_failed", e.to_string()))?;
    Ok(api_ok(serde_json::to_value(source).unwrap_or_default()))
}

async fn get_local_api_config(
    State(state): State<ApiState>,
) -> ApiResult<Json<serde_json::Value>> {
    let config = state.config.read();
    Ok(api_ok(serde_json::to_value(config.mcp.clone()).unwrap_or_default()))
}

async fn update_local_api_config(
    State(state): State<ApiState>,
    Json(local_api): Json<crate::config::schema::McpConfig>,
) -> ApiResult<Json<serde_json::Value>> {
    {
        let mut config = state.config.write();
        config.mcp = local_api.clone();
        crate::config::save_config(&config)
            .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "config_save_failed", e.to_string()))?;
    }

    {
        let mut guard = state.api_server_abort.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(abort_fn) = guard.take() {
            abort_fn();
        }
    }

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    if local_api.enabled && local_api.api_port > 0 {
        let state_clone = Arc::clone(&state);
        let api_key = local_api.api_key.clone();
        let port = local_api.api_port;
        let handle = tokio::spawn(async move {
            if let Err(e) = crate::api::run_server(state_clone, port, api_key).await {
                tracing::error!("API server error after restart: {}", e);
            }
        });
        let mut guard = state.api_server_abort.lock().unwrap_or_else(|e| e.into_inner());
        *guard = Some(Box::new(move || handle.abort()));
    }

    Ok(api_ok(serde_json::to_value(local_api).unwrap_or_default()))
}

// ---------------------------------------------------------------------------
// Recordings
// ---------------------------------------------------------------------------

async fn list_saved_recordings(
    State(state): State<ApiState>,
) -> ApiResult<Json<serde_json::Value>> {
    Ok(api_ok(serde_json::json!({ "recordings": state.recording_manager.list() })))
}

async fn get_saved_recording(
    AxumPath(id): AxumPath<String>,
    State(state): State<ApiState>,
) -> ApiResult<Json<serde_json::Value>> {
    state
        .recording_manager
        .get(&id)
        .map(|recording| api_ok(serde_json::to_value(recording).unwrap_or_default()))
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "recording_not_found", "Recording not found"))
}

async fn save_recording(
    State(state): State<ApiState>,
    Json(recording): Json<crate::recording::Recording>,
) -> Result<(StatusCode, Json<serde_json::Value>), ApiError> {
    state
        .recording_manager
        .save(recording.clone())
        .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "recording_save_failed", e.to_string()))?;
    Ok((StatusCode::CREATED, api_ok(serde_json::to_value(recording).unwrap_or_default())))
}

async fn delete_saved_recording(
    AxumPath(id): AxumPath<String>,
    State(state): State<ApiState>,
) -> Result<StatusCode, (StatusCode, String)> {
    state
        .recording_manager
        .delete(&id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(StatusCode::NO_CONTENT)
}

async fn play_recording(
    AxumPath((id, profile_id)): AxumPath<(String, String)>,
    State(state): State<ApiState>,
) -> ApiResult<Json<serde_json::Value>> {
    let recording = state
        .recording_manager
        .get(&id)
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "recording_not_found", "Recording not found"))?;

    let result = crate::recording::playback::play_recording_on_profile(
        Arc::clone(&state),
        recording,
        profile_id,
    )
    .await
    .map_err(|message| ApiError::new(StatusCode::BAD_REQUEST, "playback_failed", message))?;

    Ok(api_ok(serde_json::to_value(result).unwrap_or_default()))
}

async fn start_recording(
    AxumPath(profile_id): AxumPath<String>,
    State(state): State<ApiState>,
) -> ApiResult<Json<serde_json::Value>> {
    let session_id = state
        .recording_session_manager
        .start_session(profile_id.clone())
        .map_err(|e| ApiError::new(StatusCode::BAD_REQUEST, "start_recording_failed", e))?;

    // Start manual recording by injecting JS listeners
    tracing::info!("Starting manual recording for profile {}", profile_id);
    if let Some(cdp_port) = state.process_manager.get_cdp_port(&profile_id) {
        tracing::info!("Got CDP port: {}", cdp_port);
        match state.session_manager.get_client(&profile_id, cdp_port).await {
            Ok(handle) => {
                let client = handle.lock().await;
                client.clear_console_logs().await;
                match client.start_manual_recording().await {
                    Ok(_) => tracing::info!("Manual recording listeners injected successfully"),
                    Err(e) => tracing::error!("Failed to inject recording listeners: {}", e),
                }
            }
            Err(e) => {
                tracing::error!("Failed to get CDP client: {}", e);
            }
        }
    } else {
        tracing::warn!("No CDP port found for profile {}", profile_id);
    }

    // Emit event for UI update
    state.emit("recording-status-changed");

    Ok(api_ok(serde_json::json!({ "session_id": session_id })))
}

async fn stop_recording(
    AxumPath(session_id): AxumPath<String>,
    State(state): State<ApiState>,
) -> ApiResult<Json<serde_json::Value>> {
    // Find the profile_id for this session
    let active_sessions = state.recording_session_manager.get_active_sessions();
    let profile_id = active_sessions
        .iter()
        .find(|(_, sid)| *sid == &session_id)
        .map(|(pid, _)| pid.clone());

    let profile_id = profile_id.ok_or_else(|| {
        ApiError::new(
            StatusCode::NOT_FOUND,
            "session_not_found",
            format!("Recording session {} not found", session_id),
        )
    })?;

    // Stop manual recording and extract events from console log
    if let Some(cdp_port) = state.process_manager.get_cdp_port(&profile_id) {
        if let Ok(handle) = state.session_manager.get_client(&profile_id, cdp_port).await {
            let client = handle.lock().await;

            // First, extract manual recording events from console log
            let console_log = client.get_console_log().await;
            tracing::info!("Extracting events from {} console log entries", console_log.len());

            let mut event_count = 0;
            for entry in &console_log {
                if let Some(args) = entry.get("args").and_then(|a| a.as_array()) {
                    if args.len() >= 2 && args[0] == "__BROWSION_EVENT__" {
                        if args[1].is_string() {
                            let event_str = args[1].as_str().unwrap_or("");
                            if let Ok(event_data) = serde_json::from_str::<serde_json::Value>(event_str) {
                                if let (Some(event_type), Some(data)) = (
                                    event_data.get("type").and_then(|t| t.as_str()),
                                    event_data.get("data")
                                ) {
                                    if let Some((at, params)) = map_manual_event_to_action(event_type, data) {
                                        match state.recording_session_manager.add_action(
                                            &profile_id,
                                            at,
                                            params,
                                        ) {
                                            Ok(_) => event_count += 1,
                                            Err(e) => tracing::error!("Failed to add action: {}", e),
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            tracing::info!("Extracted {} manual recording events", event_count);

            // Now stop the recording (this will send __BROWSION_STOPPED__)
            let _ = client.stop_manual_recording().await;
        }
    }

    let mut recording = state
        .recording_session_manager
        .stop_session(&profile_id)
        .map_err(|e| ApiError::new(StatusCode::BAD_REQUEST, "stop_recording_failed", e))?;

    recording.actions = normalize_recording_actions(recording.actions);
    state
        .recording_manager
        .save(recording.clone())
        .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "save_recording_failed", e.to_string()))?;

    // Emit event for UI update
    state.emit("recording-status-changed");

    Ok(api_ok(serde_json::to_value(recording).unwrap()))
}

async fn get_recording_status(
    AxumPath(id): AxumPath<String>,
    State(state): State<ApiState>,
) -> ApiResult<Json<serde_json::Value>> {
    let info = state
        .recording_session_manager
        .get_session_info(&id);

    match info {
        Some(info) => Ok(api_ok(serde_json::json!({
            "is_recording": info.is_recording,
            "session_id": info.id,
            "action_count": info.action_count,
        }))),
        None => Ok(api_ok(serde_json::json!({
            "is_recording": false,
            "session_id": serde_json::Value::Null,
            "action_count": 0,
        }))),
    }
}

// ---------------------------------------------------------------------------
// Profile CRUD
// ---------------------------------------------------------------------------

async fn list_profiles(
    State(state): State<ApiState>,
) -> ApiResult<Json<serde_json::Value>> {
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
    Ok(api_ok(serde_json::json!({ "profiles": profiles })))
}

async fn get_profile(
    State(state): State<ApiState>,
    AxumPath(id): AxumPath<String>,
) -> ApiResult<Json<serde_json::Value>> {
    let config = state.config.read();
    let profile = config
        .profiles
        .iter()
        .find(|p| p.id == id)
        .cloned()
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "profile_not_found", "Profile not found"))?;
    Ok(api_ok(serde_json::to_value(profile).unwrap_or_default()))
}

async fn add_profile(
    State(state): State<ApiState>,
    Json(profile): Json<BrowserProfile>,
) -> Result<(StatusCode, Json<serde_json::Value>), ApiError> {
    validation::validate_profile(&profile)
        .map_err(|e| ApiError::new(StatusCode::BAD_REQUEST, "invalid_profile", e.to_string()))?;
    let mut config = state.config.write();
    if config.profiles.iter().any(|p| p.id == profile.id) {
        return Err(ApiError::new(StatusCode::CONFLICT, "profile_id_exists", "Profile ID already exists"));
    }
    config.profiles.push(profile.clone());
    crate::config::save_config(&config)
        .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "config_save_failed", e.to_string()))?;
    drop(config);
    state.emit("profiles-changed");
    Ok((StatusCode::CREATED, api_ok(serde_json::to_value(profile).unwrap_or_default())))
}

async fn update_profile(
    State(state): State<ApiState>,
    AxumPath(id): AxumPath<String>,
    Json(profile): Json<BrowserProfile>,
) -> ApiResult<Json<serde_json::Value>> {
    if profile.id != id {
        return Err(ApiError::new(StatusCode::BAD_REQUEST, "profile_id_mismatch", "ID in path and body must match"));
    }
    validation::validate_profile(&profile)
        .map_err(|e| ApiError::new(StatusCode::BAD_REQUEST, "invalid_profile", e.to_string()))?;
    let mut config = state.config.write();
    let pos = config
        .profiles
        .iter()
        .position(|p| p.id == id)
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "profile_not_found", "Profile not found"))?;
    config.profiles[pos] = profile.clone();
    crate::config::save_config(&config)
        .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "config_save_failed", e.to_string()))?;
    drop(config);
    state.emit("profiles-changed");
    Ok(api_ok(serde_json::to_value(profile).unwrap_or_default()))
}

async fn delete_profile(
    State(state): State<ApiState>,
    AxumPath(id): AxumPath<String>,
) -> Result<StatusCode, ApiError> {
    if state.process_manager.is_running(&id) {
        return Err(ApiError::new(StatusCode::CONFLICT, "profile_running", "Cannot delete profile while it is running"));
    }
    let mut config = state.config.write();
    let before = config.profiles.len();
    config.profiles.retain(|p| p.id != id);
    if config.profiles.len() == before {
        return Err(ApiError::new(StatusCode::NOT_FOUND, "profile_not_found", "Profile not found"));
    }
    crate::config::save_config(&config)
        .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "config_save_failed", e.to_string()))?;
    drop(config);
    state.emit("profiles-changed");
    Ok(StatusCode::NO_CONTENT)
}

// ---------------------------------------------------------------------------
// Browser lifecycle
// ---------------------------------------------------------------------------

#[derive(serde::Serialize)]
pub struct LaunchResponse {
    pub pid: u32,
    pub cdp_port: u16,
}

async fn launch_profile(
    State(state): State<ApiState>,
    AxumPath(profile_id): AxumPath<String>,
) -> ApiResult<Json<serde_json::Value>> {
    let config = state.config.read().clone();
    let _profile = config
        .profiles
        .iter()
        .find(|p| p.id == profile_id)
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "profile_not_found", "Profile not found"))?;

    // If already running, return existing process info (attach to existing session)
    if state.process_manager.is_running(&profile_id) {
        if let Some(info) = state.process_manager.get_process_info(&profile_id) {
            if let Some(cdp_port) = info.cdp_port {
                tracing::info!("Attaching to existing browser session for profile {} (pid: {}, cdp_port: {})",
                    profile_id, info.pid, cdp_port);
                return Ok(api_ok(serde_json::to_value(LaunchResponse {
                    pid: info.pid,
                    cdp_port
                }).unwrap_or_default()));
            }
        }
        return Err(ApiError::new(
            StatusCode::CONFLICT,
            "profile_already_running",
            "Profile is already running but CDP port is unknown",
        ));
    }

    let chrome_path = get_effective_chrome_path_from_config(&config)
        .await
        .map_err(|e| ApiError::new(StatusCode::BAD_REQUEST, "invalid_browser_source", e))?;
    let (pid, cdp_port) = state
        .process_manager
        .launch_profile(&profile_id, &config, &chrome_path)
        .await
        .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "launch_failed", e.to_string()))?;
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
    state.broadcast_ws(ws::WsEvent::BrowserStatusChanged {
        profile_id: profile_id.clone(),
        running: true,
    });
    Ok(api_ok(serde_json::to_value(LaunchResponse { pid, cdp_port }).unwrap_or_default()))
}

async fn kill_profile(
    State(state): State<ApiState>,
    AxumPath(profile_id): AxumPath<String>,
) -> Result<StatusCode, ApiError> {
    state.session_manager.disconnect(&profile_id).await;
    state
        .process_manager
        .kill_profile(&profile_id)
        .await
        .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "kill_failed", e.to_string()))?;
    // Remove from persisted sessions
    let kill_id = profile_id.clone();
    tokio::spawn(async move {
        if let Err(e) = crate::process::sessions_persist::remove_session(&kill_id).await {
            tracing::warn!("Failed to remove persisted session for {}: {}", kill_id, e);
        }
    });
    state.emit("browser-status-changed");
    state.broadcast_ws(ws::WsEvent::BrowserStatusChanged {
        profile_id: profile_id.clone(),
        running: false,
    });
    Ok(StatusCode::NO_CONTENT)
}

/// Request body for registering an externally-launched browser
#[derive(serde::Deserialize)]
struct RegisterExternalRequest {
    profile_id: String,
    pid: u32,
    cdp_port: u16,
}

async fn register_external_profile(
    State(state): State<ApiState>,
    Json(req): Json<RegisterExternalRequest>,
) -> ApiResult<Json<serde_json::Value>> {
    // Verify profile exists
    let config = state.config.read().clone();
    let _profile = config
        .profiles
        .iter()
        .find(|p| p.id == req.profile_id)
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "profile_not_found", "Profile not found"))?;

    // Verify CDP port is accessible
    let url = format!("http://127.0.0.1:{}/json/version", req.cdp_port);
    let response = reqwest::get(&url)
        .await
        .map_err(|e| ApiError::new(StatusCode::BAD_REQUEST, "cdp_unreachable", format!("Failed to connect to CDP port: {}", e)))?;

    if !response.status().is_success() {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "invalid_cdp_port",
            "CDP port is not accessible or not a Chrome browser",
        ));
    }

    // Check if already running (prevent duplicate registration)
    if state.process_manager.is_running(&req.profile_id) {
        // Return existing info if it's the same profile
        if let Some(info) = state.process_manager.get_process_info(&req.profile_id) {
            if let Some(cdp_port) = info.cdp_port {
                return Ok(api_ok(serde_json::to_value(LaunchResponse {
                    pid: info.pid,
                    cdp_port
                }).unwrap_or_default()));
            }
        }
        return Err(ApiError::new(
            StatusCode::CONFLICT,
            "profile_already_running",
            "Profile is already running with different settings",
        ));
    }

    // Register the external session
    state
        .process_manager
        .register_external(&req.profile_id, req.pid, req.cdp_port);

    // Persist session for reconnect across Tauri restarts
    let profile_id = req.profile_id.clone();
    let pid = req.pid;
    let cdp_port = req.cdp_port;
    tokio::spawn(async move {
        if let Err(e) = crate::process::sessions_persist::save_session(&profile_id, pid, cdp_port).await {
            tracing::warn!("Failed to persist external session for {}: {}", profile_id, e);
        }
    });

    state.emit("browser-status-changed");
    state.broadcast_ws(ws::WsEvent::BrowserStatusChanged {
        profile_id: req.profile_id.clone(),
        running: true,
    });

    tracing::info!(
        "Registered external browser: profile={} pid={} cdp_port={}",
        req.profile_id,
        req.pid,
        req.cdp_port
    );

    Ok(api_ok(serde_json::to_value(LaunchResponse {
        pid: req.pid,
        cdp_port: req.cdp_port
    }).unwrap_or_default()))
}

#[derive(serde::Serialize)]
struct RunningBrowser {
    profile_id: String,
    pid: u32,
    cdp_port: Option<u16>,
    launched_at: u64,
}

async fn get_running_browsers(State(state): State<ApiState>) -> Json<serde_json::Value> {
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
    api_ok(serde_json::json!({ "browsers": browsers }))
}

// ---------------------------------------------------------------------------
// Browser control: request/response types
// ---------------------------------------------------------------------------

#[derive(serde::Deserialize)]
struct NavigateReq {
    url: String,
}

#[derive(serde::Deserialize)]
struct ClickReq {
    selector: String,
}

#[derive(serde::Deserialize)]
struct TypeTextReq {
    selector: String,
    text: String,
}

#[derive(serde::Deserialize)]
struct PressKeyReq {
    key: String,
}

#[derive(serde::Deserialize)]
struct ScrollReq {
    direction: String,
    #[serde(default = "default_scroll_amount")]
    amount: u32,
}

#[derive(serde::Deserialize)]
struct ScrollElementReq {
    selector: String,
    #[serde(default)]
    delta_x: f64,
    #[serde(default)]
    delta_y: f64,
}

fn default_scroll_amount() -> u32 {
    500
}

#[derive(serde::Deserialize)]
struct WaitForReq {
    selector: String,
    #[serde(default = "default_timeout_ms")]
    timeout_ms: u64,
}

fn default_timeout_ms() -> u64 {
    5000
}

#[derive(serde::Deserialize)]
struct ExtractReq {
    selectors: HashMap<String, String>,
}

#[derive(serde::Deserialize)]
struct EvaluateReq {
    expression: String,
}

#[derive(serde::Deserialize)]
struct NavigateWaitReq {
    url: String,
    #[serde(default = "default_wait_until")]
    wait_until: String,
    #[serde(default = "default_nav_timeout_ms")]
    timeout_ms: u64,
}

fn default_wait_until() -> String {
    "load".to_string()
}

fn default_nav_timeout_ms() -> u64 {
    15000
}

#[derive(serde::Deserialize)]
struct SlowTypeReq {
    selector: String,
    text: String,
    #[serde(default = "default_key_delay_ms")]
    delay_ms: u64,
}

fn default_key_delay_ms() -> u64 {
    50
}

#[derive(serde::Deserialize)]
struct UploadFileReq {
    selector: String,
    file_path: String,
}

#[derive(serde::Deserialize)]
struct SelectOptionReq {
    selector: String,
    value: String,
}

#[derive(serde::Deserialize)]
struct WaitForNavReq {
    #[serde(default = "default_nav_timeout_ms")]
    timeout_ms: u64,
}

#[derive(serde::Deserialize)]
struct RefReq {
    ref_id: String,
}

#[derive(serde::Deserialize)]
struct TypeRefReq {
    ref_id: String,
    text: String,
}

// ── Phase 6: Flatten Mode request structs ─────────────────────────────────────

#[derive(serde::Deserialize)]
struct WaitNewTabReq {
    #[serde(default = "default_timeout_ms")]
    timeout_ms: u64,
}

#[derive(serde::Deserialize)]
struct InterceptBlockReq {
    url_pattern: String,
}

#[derive(serde::Deserialize)]
struct InterceptMockReq {
    url_pattern: String,
    status: u16,
    body: String,
    #[serde(default = "default_content_type")]
    content_type: String,
}
fn default_content_type() -> String { "application/json".to_string() }

#[derive(serde::Deserialize)]
struct PdfReq {
    #[serde(default)]
    landscape: bool,
    #[serde(default = "default_print_bg")]
    print_background: bool,
    #[serde(default = "default_scale")]
    scale: f64,
}
fn default_print_bg() -> bool { true }
fn default_scale() -> f64 { 1.0 }

#[derive(serde::Deserialize)]
struct TapReq {
    selector: String,
}

#[derive(serde::Deserialize)]
struct SwipeReq {
    selector: String,
    direction: String,
    #[serde(default = "default_swipe_distance")]
    distance: f64,
}
fn default_swipe_distance() -> f64 { 300.0 }

#[derive(serde::Deserialize)]
struct SwitchFrameReq {
    frame_id: String,
}

// ---------------------------------------------------------------------------
// Macro to reduce boilerplate for CDP endpoint error mapping
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct ApiError {
    status: StatusCode,
    code: String,
    message: String,
    details: serde_json::Map<String, serde_json::Value>,
}

impl ApiError {
    fn new(status: StatusCode, code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            status,
            code: code.into(),
            message: message.into(),
            details: serde_json::Map::new(),
        }
    }

    fn with_detail(mut self, key: &str, value: serde_json::Value) -> Self {
        self.details.insert(key.to_string(), value);
        self
    }
}

impl From<(StatusCode, String)> for ApiError {
    fn from(value: (StatusCode, String)) -> Self {
        Self::new(value.0, "api_error", value.1)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let mut error = serde_json::json!({
            "code": self.code,
            "message": self.message,
            "status": self.status.as_u16(),
        });
        if let Some(object) = error.as_object_mut() {
            for (key, value) in self.details {
                object.insert(key, value);
            }
        }

        (
            self.status,
            Json(serde_json::json!({
                "ok": false,
                "error": error,
            })),
        )
            .into_response()
    }
}

type ApiResult<T> = Result<T, ApiError>;

fn api_ok(mut payload: serde_json::Value) -> Json<serde_json::Value> {
    if let Some(object) = payload.as_object_mut() {
        object.insert("ok".to_string(), serde_json::json!(true));
        Json(payload)
    } else {
        Json(serde_json::json!({
            "ok": true,
            "data": payload,
        }))
    }
}

fn cdp_err(e: String) -> ApiError {
    ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "browser_command_failed", e)
}

// ---------------------------------------------------------------------------
// Browser control endpoints
// ---------------------------------------------------------------------------

async fn browser_navigate(
    State(state): State<ApiState>,
    AxumPath(id): AxumPath<String>,
    Json(req): Json<NavigateReq>,
) -> ApiResult<Json<serde_json::Value>> {
    let cdp_port = require_cdp_port(&state, &id)?;
    let handle = state.session_manager.get_client(&id, cdp_port).await.map_err(cdp_err)?;
    let client = handle.lock().await;
    client.navigate(&req.url).await.map_err(cdp_err)?;
    record_browser_action(&state, &id, crate::recording::RecordedActionType::Navigate, serde_json::json!({ "url": req.url }));
    let url = client.get_url().await.map_err(cdp_err)?;
    let title = client.get_title().await.map_err(cdp_err)?;
    Ok(api_ok(serde_json::json!({ "url": url, "title": title })))
}

async fn browser_get_url(
    State(state): State<ApiState>,
    AxumPath(id): AxumPath<String>,
) -> ApiResult<Json<serde_json::Value>> {
    let cdp_port = require_cdp_port(&state, &id)?;
    let handle = state.session_manager.get_client(&id, cdp_port).await.map_err(cdp_err)?;
    let client = handle.lock().await;
    let url = client.get_url().await.map_err(cdp_err)?;
    Ok(api_ok(serde_json::json!({ "url": url })))
}

async fn browser_get_title(
    State(state): State<ApiState>,
    AxumPath(id): AxumPath<String>,
) -> ApiResult<Json<serde_json::Value>> {
    let cdp_port = require_cdp_port(&state, &id)?;
    let handle = state.session_manager.get_client(&id, cdp_port).await.map_err(cdp_err)?;
    let client = handle.lock().await;
    let title = client.get_title().await.map_err(cdp_err)?;
    Ok(api_ok(serde_json::json!({ "title": title })))
}

async fn browser_go_back(
    State(state): State<ApiState>,
    AxumPath(id): AxumPath<String>,
) -> ApiResult<Json<serde_json::Value>> {
    let cdp_port = require_cdp_port(&state, &id)?;
    let handle = state.session_manager.get_client(&id, cdp_port).await.map_err(cdp_err)?;
    let client = handle.lock().await;
    client.go_back().await.map_err(cdp_err)?;
    record_browser_action(&state, &id, crate::recording::RecordedActionType::GoBack, serde_json::json!({}));
    let url = client.get_url().await.map_err(cdp_err)?;
    let title = client.get_title().await.map_err(cdp_err)?;
    Ok(api_ok(serde_json::json!({ "url": url, "title": title })))
}

async fn browser_go_forward(
    State(state): State<ApiState>,
    AxumPath(id): AxumPath<String>,
) -> ApiResult<Json<serde_json::Value>> {
    let cdp_port = require_cdp_port(&state, &id)?;
    let handle = state.session_manager.get_client(&id, cdp_port).await.map_err(cdp_err)?;
    let client = handle.lock().await;
    client.go_forward().await.map_err(cdp_err)?;
    record_browser_action(&state, &id, crate::recording::RecordedActionType::GoForward, serde_json::json!({}));
    let url = client.get_url().await.map_err(cdp_err)?;
    let title = client.get_title().await.map_err(cdp_err)?;
    Ok(api_ok(serde_json::json!({ "url": url, "title": title })))
}

async fn browser_reload(
    State(state): State<ApiState>,
    AxumPath(id): AxumPath<String>,
) -> ApiResult<Json<serde_json::Value>> {
    let cdp_port = require_cdp_port(&state, &id)?;
    let handle = state.session_manager.get_client(&id, cdp_port).await.map_err(cdp_err)?;
    let client = handle.lock().await;
    client.reload().await.map_err(cdp_err)?;
    record_browser_action(&state, &id, crate::recording::RecordedActionType::Reload, serde_json::json!({}));
    let url = client.get_url().await.map_err(cdp_err)?;
    let title = client.get_title().await.map_err(cdp_err)?;
    Ok(api_ok(serde_json::json!({ "url": url, "title": title })))
}

async fn browser_navigate_wait(
    State(state): State<ApiState>,
    AxumPath(id): AxumPath<String>,
    Json(req): Json<NavigateWaitReq>,
) -> ApiResult<Json<serde_json::Value>> {
    let cdp_port = require_cdp_port(&state, &id)?;
    let handle = state.session_manager.get_client(&id, cdp_port).await.map_err(cdp_err)?;
    let client = handle.lock().await;
    client
        .navigate_wait(&req.url, &req.wait_until, req.timeout_ms)
        .await
        .map_err(cdp_err)?;
    record_browser_action(&state, &id, crate::recording::RecordedActionType::Navigate, serde_json::json!({ "url": req.url }));
    let url = client.get_url().await.map_err(cdp_err)?;
    let title = client.get_title().await.map_err(cdp_err)?;
    Ok(api_ok(serde_json::json!({ "url": url, "title": title })))
}

async fn browser_hover(
    State(state): State<ApiState>,
    AxumPath(id): AxumPath<String>,
    Json(req): Json<ClickReq>,
) -> ApiResult<Json<serde_json::Value>> {
    let cdp_port = require_cdp_port(&state, &id)?;
    let handle = state.session_manager.get_client(&id, cdp_port).await.map_err(cdp_err)?;
    let client = handle.lock().await;
    client.hover(&req.selector).await.map_err(cdp_err)?;
    record_browser_action(&state, &id, crate::recording::RecordedActionType::Hover, serde_json::json!({ "selector": req.selector }));
    Ok(api_ok(serde_json::json!({})))
}

async fn browser_double_click(
    State(state): State<ApiState>,
    AxumPath(id): AxumPath<String>,
    Json(req): Json<ClickReq>,
) -> ApiResult<Json<serde_json::Value>> {
    let cdp_port = require_cdp_port(&state, &id)?;
    let handle = state.session_manager.get_client(&id, cdp_port).await.map_err(cdp_err)?;
    let client = handle.lock().await;
    client.double_click(&req.selector).await.map_err(cdp_err)?;
    record_browser_action(&state, &id, crate::recording::RecordedActionType::DoubleClick, serde_json::json!({ "selector": req.selector }));
    Ok(api_ok(serde_json::json!({})))
}

async fn browser_right_click(
    State(state): State<ApiState>,
    AxumPath(id): AxumPath<String>,
    Json(req): Json<ClickReq>,
) -> ApiResult<Json<serde_json::Value>> {
    let cdp_port = require_cdp_port(&state, &id)?;
    let handle = state.session_manager.get_client(&id, cdp_port).await.map_err(cdp_err)?;
    let client = handle.lock().await;
    client.right_click(&req.selector).await.map_err(cdp_err)?;
    record_browser_action(&state, &id, crate::recording::RecordedActionType::RightClick, serde_json::json!({ "selector": req.selector }));
    Ok(api_ok(serde_json::json!({})))
}

async fn browser_click(
    State(state): State<ApiState>,
    AxumPath(id): AxumPath<String>,
    Json(req): Json<ClickReq>,
) -> ApiResult<Json<serde_json::Value>> {
    let cdp_port = require_cdp_port(&state, &id)?;
    let handle = state.session_manager.get_client(&id, cdp_port).await.map_err(cdp_err)?;
    let client = handle.lock().await;
    client.click(&req.selector).await.map_err(cdp_err)?;
    record_browser_action(&state, &id, crate::recording::RecordedActionType::Click, serde_json::json!({ "selector": req.selector }));
    Ok(api_ok(serde_json::json!({})))
}

async fn browser_type_text(
    State(state): State<ApiState>,
    AxumPath(id): AxumPath<String>,
    Json(req): Json<TypeTextReq>,
) -> ApiResult<Json<serde_json::Value>> {
    let cdp_port = require_cdp_port(&state, &id)?;
    let handle = state.session_manager.get_client(&id, cdp_port).await.map_err(cdp_err)?;
    let client = handle.lock().await;
    client.type_text(&req.selector, &req.text).await.map_err(cdp_err)?;
    record_browser_action(&state, &id, crate::recording::RecordedActionType::Type, serde_json::json!({ "selector": req.selector, "text": req.text }));
    Ok(api_ok(serde_json::json!({})))
}

async fn browser_press_key(
    State(state): State<ApiState>,
    AxumPath(id): AxumPath<String>,
    Json(req): Json<PressKeyReq>,
) -> ApiResult<Json<serde_json::Value>> {
    let cdp_port = require_cdp_port(&state, &id)?;
    let handle = state.session_manager.get_client(&id, cdp_port).await.map_err(cdp_err)?;
    let client = handle.lock().await;
    client.press_key(&req.key).await.map_err(cdp_err)?;
    record_browser_action(&state, &id, crate::recording::RecordedActionType::PressKey, serde_json::json!({ "key": req.key }));
    Ok(api_ok(serde_json::json!({})))
}

async fn browser_slow_type(
    State(state): State<ApiState>,
    AxumPath(id): AxumPath<String>,
    Json(req): Json<SlowTypeReq>,
) -> ApiResult<Json<serde_json::Value>> {
    let cdp_port = require_cdp_port(&state, &id)?;
    let handle = state.session_manager.get_client(&id, cdp_port).await.map_err(cdp_err)?;
    let client = handle.lock().await;
    client
        .slow_type(&req.selector, &req.text, req.delay_ms)
        .await
        .map_err(cdp_err)?;
    record_browser_action(&state, &id, crate::recording::RecordedActionType::SlowType, serde_json::json!({ "selector": req.selector, "text": req.text, "delay_ms": req.delay_ms }));
    Ok(api_ok(serde_json::json!({})))
}

async fn browser_upload_file(
    State(state): State<ApiState>,
    AxumPath(id): AxumPath<String>,
    Json(req): Json<UploadFileReq>,
) -> ApiResult<Json<serde_json::Value>> {
    let cdp_port = require_cdp_port(&state, &id)?;
    let handle = state.session_manager.get_client(&id, cdp_port).await.map_err(cdp_err)?;
    let client = handle.lock().await;
    client
        .upload_file(&req.selector, vec![req.file_path.clone()])
        .await
        .map_err(cdp_err)?;
    record_browser_action(&state, &id, crate::recording::RecordedActionType::UploadFile, serde_json::json!({ "selector": req.selector, "file_path": req.file_path }));
    Ok(Json(serde_json::json!({ "ok": true })))
}

async fn browser_wait_for_navigation(
    State(state): State<ApiState>,
    AxumPath(id): AxumPath<String>,
    Json(req): Json<WaitForNavReq>,
) -> ApiResult<Json<serde_json::Value>> {
    let cdp_port = require_cdp_port(&state, &id)?;
    let handle = state.session_manager.get_client(&id, cdp_port).await.map_err(cdp_err)?;
    let client = handle.lock().await;
    client
        .wait_for_navigation(req.timeout_ms)
        .await
        .map_err(cdp_err)?;
    let url = client.get_url().await.map_err(cdp_err)?;
    let title = client.get_title().await.map_err(cdp_err)?;
    Ok(api_ok(serde_json::json!({ "url": url, "title": title })))
}

async fn browser_ax_tree(
    State(state): State<ApiState>,
    AxumPath(id): AxumPath<String>,
) -> ApiResult<Json<serde_json::Value>> {
    let cdp_port = require_cdp_port(&state, &id)?;
    let handle = state.session_manager.get_client(&id, cdp_port).await.map_err(cdp_err)?;
    let client = handle.lock().await;
    let nodes = client.get_ax_tree().await.map_err(cdp_err)?;
    let value = serde_json::to_value(nodes)
        .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "serialization_failed", e.to_string()))?;
    Ok(api_ok(value))
}

async fn browser_page_state(
    State(state): State<ApiState>,
    AxumPath(id): AxumPath<String>,
) -> ApiResult<Json<serde_json::Value>> {
    let cdp_port = require_cdp_port(&state, &id)?;
    let handle = state.session_manager.get_client(&id, cdp_port).await.map_err(cdp_err)?;
    let client = handle.lock().await;
    let state_val = client.get_page_state().await.map_err(cdp_err)?;
    let value = serde_json::to_value(state_val)
        .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "serialization_failed", e.to_string()))?;
    Ok(api_ok(value))
}

async fn browser_click_ref(
    State(state): State<ApiState>,
    AxumPath(id): AxumPath<String>,
    Json(req): Json<RefReq>,
) -> ApiResult<Json<serde_json::Value>> {
    let cdp_port = require_cdp_port(&state, &id)?;
    let handle = state.session_manager.get_client(&id, cdp_port).await.map_err(cdp_err)?;
    let client = handle.lock().await;
    client.click_ref(&req.ref_id).await.map_err(cdp_err)?;
    Ok(api_ok(serde_json::json!({})))
}

async fn browser_type_ref(
    State(state): State<ApiState>,
    AxumPath(id): AxumPath<String>,
    Json(req): Json<TypeRefReq>,
) -> ApiResult<Json<serde_json::Value>> {
    let cdp_port = require_cdp_port(&state, &id)?;
    let handle = state.session_manager.get_client(&id, cdp_port).await.map_err(cdp_err)?;
    let client = handle.lock().await;
    client.type_ref(&req.ref_id, &req.text).await.map_err(cdp_err)?;
    Ok(api_ok(serde_json::json!({})))
}

async fn browser_focus_ref(
    State(state): State<ApiState>,
    AxumPath(id): AxumPath<String>,
    Json(req): Json<RefReq>,
) -> ApiResult<Json<serde_json::Value>> {
    let cdp_port = require_cdp_port(&state, &id)?;
    let handle = state.session_manager.get_client(&id, cdp_port).await.map_err(cdp_err)?;
    let client = handle.lock().await;
    client.focus_ref(&req.ref_id).await.map_err(cdp_err)?;
    Ok(api_ok(serde_json::json!({})))
}

async fn browser_scroll_into_view(
    State(state): State<ApiState>,
    AxumPath(id): AxumPath<String>,
    Json(req): Json<ClickReq>,
) -> ApiResult<Json<serde_json::Value>> {
    let cdp_port = require_cdp_port(&state, &id)?;
    let handle = state.session_manager.get_client(&id, cdp_port).await.map_err(cdp_err)?;
    let client = handle.lock().await;
    client.scroll_into_view(&req.selector).await.map_err(cdp_err)?;
    Ok(api_ok(serde_json::json!({})))
}

async fn browser_select_option(
    State(state): State<ApiState>,
    AxumPath(id): AxumPath<String>,
    Json(req): Json<SelectOptionReq>,
) -> ApiResult<Json<serde_json::Value>> {
    let cdp_port = require_cdp_port(&state, &id)?;
    let handle = state.session_manager.get_client(&id, cdp_port).await.map_err(cdp_err)?;
    let client = handle.lock().await;
    client.select_option(&req.selector, &req.value).await.map_err(cdp_err)?;
    Ok(api_ok(serde_json::json!({})))
}

async fn browser_scroll(
    State(state): State<ApiState>,
    AxumPath(id): AxumPath<String>,
    Json(req): Json<ScrollReq>,
) -> ApiResult<Json<serde_json::Value>> {
    let cdp_port = require_cdp_port(&state, &id)?;
    let handle = state.session_manager.get_client(&id, cdp_port).await.map_err(cdp_err)?;
    let client = handle.lock().await;
    client.scroll(&req.direction, req.amount).await.map_err(cdp_err)?;
    Ok(api_ok(serde_json::json!({})))
}

async fn browser_scroll_element(
    State(state): State<ApiState>,
    AxumPath(id): AxumPath<String>,
    Json(req): Json<ScrollElementReq>,
) -> ApiResult<Json<serde_json::Value>> {
    let cdp_port = require_cdp_port(&state, &id)?;
    let handle = state.session_manager.get_client(&id, cdp_port).await.map_err(cdp_err)?;
    let client = handle.lock().await;
    client.scroll_element(&req.selector, req.delta_x, req.delta_y).await.map_err(cdp_err)?;
    Ok(api_ok(serde_json::json!({})))
}

async fn browser_wait_for(
    State(state): State<ApiState>,
    AxumPath(id): AxumPath<String>,
    Json(req): Json<WaitForReq>,
) -> ApiResult<Json<serde_json::Value>> {
    let cdp_port = require_cdp_port(&state, &id)?;
    let handle = state.session_manager.get_client(&id, cdp_port).await.map_err(cdp_err)?;
    let client = handle.lock().await;
    client
        .wait_for_element(&req.selector, req.timeout_ms)
        .await
        .map_err(cdp_err)?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

/// Query params for screenshot
#[derive(serde::Deserialize, Default)]
struct ScreenshotQuery {
    full_page: Option<bool>,
    format: Option<String>,
    quality: Option<u32>,
}

async fn browser_screenshot(
    State(state): State<ApiState>,
    AxumPath(id): AxumPath<String>,
    axum::extract::Query(q): axum::extract::Query<ScreenshotQuery>,
) -> ApiResult<Json<serde_json::Value>> {
    let cdp_port = require_cdp_port(&state, &id)?;
    let handle = state.session_manager.get_client(&id, cdp_port).await.map_err(cdp_err)?;
    let client = handle.lock().await;
    let format = q.format.as_deref().unwrap_or("png");
    let image = client
        .screenshot(q.full_page.unwrap_or(false), format, q.quality)
        .await
        .map_err(cdp_err)?;
    Ok(api_ok(serde_json::json!({ "image": image, "format": format })))
}

/// Query params for screenshot_element
#[derive(serde::Deserialize, Default)]
struct ScreenshotElementQuery {
    selector: String,
    format: Option<String>,
    quality: Option<u32>,
}

async fn browser_screenshot_element(
    State(state): State<ApiState>,
    AxumPath(id): AxumPath<String>,
    axum::extract::Query(q): axum::extract::Query<ScreenshotElementQuery>,
) -> ApiResult<Json<serde_json::Value>> {
    let cdp_port = require_cdp_port(&state, &id)?;
    let handle = state.session_manager.get_client(&id, cdp_port).await.map_err(cdp_err)?;
    let client = handle.lock().await;
    let format = q.format.as_deref().unwrap_or("png");
    let data = client
        .screenshot_element(&q.selector, format, q.quality)
        .await
        .map_err(cdp_err)?;
    Ok(api_ok(serde_json::json!({ "data": data, "format": format })))
}

async fn browser_dom_context(
    State(state): State<ApiState>,
    AxumPath(id): AxumPath<String>,
) -> ApiResult<Json<serde_json::Value>> {
    let cdp_port = require_cdp_port(&state, &id)?;
    let handle = state.session_manager.get_client(&id, cdp_port).await.map_err(cdp_err)?;
    let client = handle.lock().await;
    let ctx = client.get_dom_context().await.map_err(cdp_err)?;
    let value =
        serde_json::to_value(ctx).map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "serialization_failed", e.to_string()))?;
    Ok(api_ok(value))
}

async fn browser_extract(
    State(state): State<ApiState>,
    AxumPath(id): AxumPath<String>,
    Json(req): Json<ExtractReq>,
) -> ApiResult<Json<serde_json::Value>> {
    let cdp_port = require_cdp_port(&state, &id)?;
    let handle = state.session_manager.get_client(&id, cdp_port).await.map_err(cdp_err)?;
    let client = handle.lock().await;
    let data = client.extract_data(&req.selectors).await.map_err(cdp_err)?;
    Ok(api_ok(data))
}

async fn browser_evaluate(
    State(state): State<ApiState>,
    AxumPath(id): AxumPath<String>,
    Json(req): Json<EvaluateReq>,
) -> ApiResult<Json<serde_json::Value>> {
    let cdp_port = require_cdp_port(&state, &id)?;
    let handle = state.session_manager.get_client(&id, cdp_port).await.map_err(cdp_err)?;
    let client = handle.lock().await;
    let result = client.evaluate_js(&req.expression).await.map_err(cdp_err)?;
    Ok(api_ok(serde_json::json!({ "result": result })))
}

// ---------------------------------------------------------------------------
// Advanced: Tabs
// ---------------------------------------------------------------------------

#[derive(serde::Deserialize)]
struct NewTabReq {
    #[serde(default = "default_new_tab_url")]
    url: String,
}

fn default_new_tab_url() -> String {
    "about:blank".to_string()
}

#[derive(serde::Deserialize)]
struct TabIdReq {
    target_id: String,
}

async fn browser_list_tabs(
    State(state): State<ApiState>,
    AxumPath(id): AxumPath<String>,
) -> ApiResult<Json<serde_json::Value>> {
    let cdp_port = require_cdp_port(&state, &id)?;
    let handle = state.session_manager.get_client(&id, cdp_port).await.map_err(cdp_err)?;
    let client = handle.lock().await;
    let tabs = client.list_tabs().await.map_err(cdp_err)?;
    let value = serde_json::to_value(tabs)
        .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "serialization_failed", e.to_string()))?;
    Ok(api_ok(value))
}

async fn browser_new_tab(
    State(state): State<ApiState>,
    AxumPath(id): AxumPath<String>,
    Json(req): Json<NewTabReq>,
) -> ApiResult<Json<serde_json::Value>> {
    let cdp_port = require_cdp_port(&state, &id)?;
    let handle = state.session_manager.get_client(&id, cdp_port).await.map_err(cdp_err)?;
    let client = handle.lock().await;
    let tab = client.new_tab(&req.url).await.map_err(cdp_err)?;
    let value = serde_json::to_value(tab)
        .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "serialization_failed", e.to_string()))?;
    Ok(api_ok(value))
}

async fn browser_switch_tab(
    State(state): State<ApiState>,
    AxumPath(id): AxumPath<String>,
    Json(req): Json<TabIdReq>,
) -> ApiResult<Json<serde_json::Value>> {
    let cdp_port = require_cdp_port(&state, &id)?;
    let handle = state.session_manager.get_client(&id, cdp_port).await.map_err(cdp_err)?;
    let client = handle.lock().await;
    client.switch_tab(&req.target_id).await.map_err(cdp_err)?;
    Ok(api_ok(serde_json::json!({})))
}

async fn browser_close_tab(
    State(state): State<ApiState>,
    AxumPath(id): AxumPath<String>,
    Json(req): Json<TabIdReq>,
) -> ApiResult<Json<serde_json::Value>> {
    let cdp_port = require_cdp_port(&state, &id)?;
    let handle = state.session_manager.get_client(&id, cdp_port).await.map_err(cdp_err)?;
    let client = handle.lock().await;
    client.close_tab(&req.target_id).await.map_err(cdp_err)?;
    Ok(api_ok(serde_json::json!({})))
}

// ---------------------------------------------------------------------------
// Advanced: Cookies
// ---------------------------------------------------------------------------

#[derive(serde::Deserialize)]
struct SetCookieReq {
    name: String,
    value: String,
    domain: String,
    #[serde(default = "default_cookie_path")]
    path: String,
}

fn default_cookie_path() -> String {
    "/".to_string()
}

async fn browser_get_cookies(
    State(state): State<ApiState>,
    AxumPath(id): AxumPath<String>,
) -> ApiResult<Json<serde_json::Value>> {
    let cdp_port = require_cdp_port(&state, &id)?;
    let handle = state.session_manager.get_client(&id, cdp_port).await.map_err(cdp_err)?;
    let client = handle.lock().await;
    let cookies = client.get_cookies().await.map_err(cdp_err)?;
    let value = serde_json::to_value(cookies)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(value))
}

async fn browser_set_cookie(
    State(state): State<ApiState>,
    AxumPath(id): AxumPath<String>,
    Json(req): Json<SetCookieReq>,
) -> ApiResult<Json<serde_json::Value>> {
    let cdp_port = require_cdp_port(&state, &id)?;
    let handle = state.session_manager.get_client(&id, cdp_port).await.map_err(cdp_err)?;
    let client = handle.lock().await;
    client
        .set_cookie(&req.name, &req.value, &req.domain, &req.path)
        .await
        .map_err(cdp_err)?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

async fn browser_delete_cookies(
    State(state): State<ApiState>,
    AxumPath(id): AxumPath<String>,
) -> ApiResult<Json<serde_json::Value>> {
    let cdp_port = require_cdp_port(&state, &id)?;
    let handle = state.session_manager.get_client(&id, cdp_port).await.map_err(cdp_err)?;
    let client = handle.lock().await;
    client.delete_cookies().await.map_err(cdp_err)?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

// ---------------------------------------------------------------------------
// Advanced: Console
// ---------------------------------------------------------------------------

async fn browser_get_console_logs(
    State(state): State<ApiState>,
    AxumPath(id): AxumPath<String>,
) -> ApiResult<Json<serde_json::Value>> {
    let cdp_port = require_cdp_port(&state, &id)?;
    let handle = state.session_manager.get_client(&id, cdp_port).await.map_err(cdp_err)?;
    let client = handle.lock().await;
    let logs = client.get_console_logs().await.map_err(cdp_err)?;
    Ok(api_ok(serde_json::json!({ "logs": logs })))
}

async fn browser_enable_console(
    State(state): State<ApiState>,
    AxumPath(id): AxumPath<String>,
) -> ApiResult<Json<serde_json::Value>> {
    let cdp_port = require_cdp_port(&state, &id)?;
    let handle = state.session_manager.get_client(&id, cdp_port).await.map_err(cdp_err)?;
    let client = handle.lock().await;
    client.enable_console_capture().await.map_err(cdp_err)?;
    Ok(api_ok(serde_json::json!({})))
}

// ---------------------------------------------------------------------------
// New handlers: dialog, click_at, drag, network_log, wait_for_text
// ---------------------------------------------------------------------------

#[derive(serde::Deserialize)]
struct HandleDialogReq {
    action: String,           // "accept" | "dismiss"
    prompt_text: Option<String>,
}

async fn browser_handle_dialog(
    State(state): State<ApiState>,
    AxumPath(id): AxumPath<String>,
    Json(body): Json<HandleDialogReq>,
) -> ApiResult<Json<serde_json::Value>> {
    let cdp_port = require_cdp_port(&state, &id)?;
    let handle = state.session_manager.get_client(&id, cdp_port).await.map_err(cdp_err)?;
    let client = handle.lock().await;
    client
        .handle_dialog(&body.action, body.prompt_text.as_deref())
        .await
        .map_err(cdp_err)?;
    Ok(api_ok(serde_json::json!({})))
}

#[derive(serde::Deserialize)]
struct ClickAtReq {
    x: f64,
    y: f64,
}

async fn browser_click_at(
    State(state): State<ApiState>,
    AxumPath(id): AxumPath<String>,
    Json(body): Json<ClickAtReq>,
) -> ApiResult<Json<serde_json::Value>> {
    let cdp_port = require_cdp_port(&state, &id)?;
    let handle = state.session_manager.get_client(&id, cdp_port).await.map_err(cdp_err)?;
    let client = handle.lock().await;
    client.click_at(body.x, body.y).await.map_err(cdp_err)?;
    Ok(api_ok(serde_json::json!({})))
}

#[derive(serde::Deserialize)]
struct DragReq {
    from_selector: String,
    to_selector: String,
}

async fn browser_drag(
    State(state): State<ApiState>,
    AxumPath(id): AxumPath<String>,
    Json(body): Json<DragReq>,
) -> ApiResult<Json<serde_json::Value>> {
    let cdp_port = require_cdp_port(&state, &id)?;
    let handle = state.session_manager.get_client(&id, cdp_port).await.map_err(cdp_err)?;
    let client = handle.lock().await;
    client.drag(&body.from_selector, &body.to_selector).await.map_err(cdp_err)?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

async fn browser_network_log(
    State(state): State<ApiState>,
    AxumPath(id): AxumPath<String>,
) -> ApiResult<Json<serde_json::Value>> {
    let cdp_port = require_cdp_port(&state, &id)?;
    let handle = state.session_manager.get_client(&id, cdp_port).await.map_err(cdp_err)?;
    let client = handle.lock().await;
    let log = client.get_network_log().await;
    Ok(api_ok(serde_json::json!({ "entries": log, "count": log.len() })))
}

async fn browser_clear_network_log(
    State(state): State<ApiState>,
    AxumPath(id): AxumPath<String>,
) -> ApiResult<Json<serde_json::Value>> {
    let cdp_port = require_cdp_port(&state, &id)?;
    let handle = state.session_manager.get_client(&id, cdp_port).await.map_err(cdp_err)?;
    let client = handle.lock().await;
    client.clear_network_log().await;
    Ok(api_ok(serde_json::json!({})))
}

#[derive(serde::Deserialize)]
struct WaitForTextReq {
    text: String,
    timeout_ms: Option<u64>,
}

#[derive(serde::Deserialize)]
struct EmulateReq {
    // Viewport
    width: Option<u32>,
    height: Option<u32>,
    device_scale_factor: Option<f64>,
    mobile: Option<bool>,
    // User agent
    user_agent: Option<String>,
    // Geolocation
    latitude: Option<f64>,
    longitude: Option<f64>,
    accuracy: Option<f64>,
}

async fn browser_wait_for_text(
    State(state): State<ApiState>,
    AxumPath(id): AxumPath<String>,
    Json(body): Json<WaitForTextReq>,
) -> ApiResult<Json<serde_json::Value>> {
    let cdp_port = require_cdp_port(&state, &id)?;
    let handle = state.session_manager.get_client(&id, cdp_port).await.map_err(cdp_err)?;
    let client = handle.lock().await;
    client
        .wait_for_text(&body.text, body.timeout_ms.unwrap_or(30000))
        .await
        .map_err(cdp_err)?;
    Ok(api_ok(serde_json::json!({})))
}

#[derive(serde::Deserialize)]
struct WaitForUrlReq {
    pattern: String,
    #[serde(default = "default_timeout_ms")]
    timeout_ms: u64,
}

async fn browser_wait_for_url(
    State(state): State<ApiState>,
    AxumPath(id): AxumPath<String>,
    Json(req): Json<WaitForUrlReq>,
) -> ApiResult<Json<serde_json::Value>> {
    let cdp_port = require_cdp_port(&state, &id)?;
    let handle = state.session_manager.get_client(&id, cdp_port).await.map_err(cdp_err)?;
    let client = handle.lock().await;
    let url = client.wait_for_url(&req.pattern, req.timeout_ms).await.map_err(cdp_err)?;
    Ok(api_ok(serde_json::json!({ "url": url })))
}

async fn browser_emulate(
    State(state): State<ApiState>,
    AxumPath(id): AxumPath<String>,
    Json(req): Json<EmulateReq>,
) -> ApiResult<Json<serde_json::Value>> {
    let cdp_port = require_cdp_port(&state, &id)?;
    let handle = state.session_manager.get_client(&id, cdp_port).await.map_err(cdp_err)?;
    let client = handle.lock().await;

    if req.width.is_some() || req.height.is_some() {
        let w = req.width.unwrap_or(1280);
        let h = req.height.unwrap_or(800);
        let dpr = req.device_scale_factor.unwrap_or(1.0);
        let mobile = req.mobile.unwrap_or(false);
        client.set_viewport(w, h, dpr, mobile).await.map_err(cdp_err)?;
    }
    if let Some(ua) = &req.user_agent {
        client.set_user_agent(ua).await.map_err(cdp_err)?;
    }
    if let (Some(lat), Some(lon)) = (req.latitude, req.longitude) {
        let accuracy = req.accuracy.unwrap_or(100.0);
        client.set_geolocation(lat, lon, accuracy).await.map_err(cdp_err)?;
    }

    Ok(api_ok(serde_json::json!({})))
}

// ---------------------------------------------------------------------------
// Advanced: Storage (localStorage / sessionStorage)
// ---------------------------------------------------------------------------

#[derive(serde::Deserialize, Default)]
struct StorageQuery {
    #[serde(default = "default_storage_type")]
    storage_type: String,
}

#[derive(serde::Deserialize)]
struct SetStorageReq {
    #[serde(default = "default_storage_type")]
    storage_type: String,
    key: String,
    value: String,
}

#[derive(serde::Deserialize)]
struct ClearStorageReq {
    #[serde(default = "default_storage_type")]
    storage_type: String,
    key: Option<String>,
}

fn default_storage_type() -> String { "local".to_string() }

async fn browser_get_storage(
    State(state): State<ApiState>,
    AxumPath(id): AxumPath<String>,
    axum::extract::Query(q): axum::extract::Query<StorageQuery>,
) -> ApiResult<Json<serde_json::Value>> {
    let cdp_port = require_cdp_port(&state, &id)?;
    let handle = state.session_manager.get_client(&id, cdp_port).await.map_err(cdp_err)?;
    let client = handle.lock().await;
    let data = client.get_storage(&q.storage_type).await.map_err(cdp_err)?;
    Ok(api_ok(serde_json::json!({ "storage": data, "type": q.storage_type })))
}

async fn browser_set_storage(
    State(state): State<ApiState>,
    AxumPath(id): AxumPath<String>,
    Json(req): Json<SetStorageReq>,
) -> ApiResult<Json<serde_json::Value>> {
    let cdp_port = require_cdp_port(&state, &id)?;
    let handle = state.session_manager.get_client(&id, cdp_port).await.map_err(cdp_err)?;
    let client = handle.lock().await;
    client.set_storage_item(&req.storage_type, &req.key, &req.value).await.map_err(cdp_err)?;
    Ok(api_ok(serde_json::json!({})))
}

async fn browser_clear_storage(
    State(state): State<ApiState>,
    AxumPath(id): AxumPath<String>,
    Json(req): Json<ClearStorageReq>,
) -> ApiResult<Json<serde_json::Value>> {
    let cdp_port = require_cdp_port(&state, &id)?;
    let handle = state.session_manager.get_client(&id, cdp_port).await.map_err(cdp_err)?;
    let client = handle.lock().await;
    match &req.key {
        Some(key) => client.remove_storage_item(&req.storage_type, key).await.map_err(cdp_err)?,
        None => client.clear_storage(&req.storage_type).await.map_err(cdp_err)?,
    }
    Ok(api_ok(serde_json::json!({})))
}

// ---------------------------------------------------------------------------
// Phase 6: Flatten Mode - wait_for_new_tab, page_text, intercept, PDF, touch, frames
// ---------------------------------------------------------------------------

async fn browser_wait_for_new_tab(
    State(state): State<ApiState>,
    AxumPath(id): AxumPath<String>,
    Json(req): Json<WaitNewTabReq>,
) -> ApiResult<Json<serde_json::Value>> {
    let cdp_port = require_cdp_port(&state, &id)?;
    let handle = state.session_manager.get_client(&id, cdp_port).await.map_err(cdp_err)?;
    let client = handle.lock().await;
    let target_id = client.wait_for_new_tab(req.timeout_ms).await.map_err(cdp_err)?;
    Ok(api_ok(serde_json::json!({ "target_id": target_id })))
}

async fn browser_get_page_text(
    State(state): State<ApiState>,
    AxumPath(id): AxumPath<String>,
) -> ApiResult<Json<serde_json::Value>> {
    let cdp_port = require_cdp_port(&state, &id)?;
    let handle = state.session_manager.get_client(&id, cdp_port).await.map_err(cdp_err)?;
    let client = handle.lock().await;
    let text = client.get_page_text().await.map_err(cdp_err)?;
    Ok(api_ok(serde_json::json!({ "text": text, "length": text.len() })))
}

async fn browser_intercept_block(
    State(state): State<ApiState>,
    AxumPath(id): AxumPath<String>,
    Json(req): Json<InterceptBlockReq>,
) -> ApiResult<Json<serde_json::Value>> {
    let cdp_port = require_cdp_port(&state, &id)?;
    let handle = state.session_manager.get_client(&id, cdp_port).await.map_err(cdp_err)?;
    let client = handle.lock().await;
    client.block_url(&req.url_pattern).await.map_err(cdp_err)?;
    Ok(Json(serde_json::json!({ "ok": true, "rule": "block", "pattern": req.url_pattern })))
}

async fn browser_intercept_mock(
    State(state): State<ApiState>,
    AxumPath(id): AxumPath<String>,
    Json(req): Json<InterceptMockReq>,
) -> ApiResult<Json<serde_json::Value>> {
    let cdp_port = require_cdp_port(&state, &id)?;
    let handle = state.session_manager.get_client(&id, cdp_port).await.map_err(cdp_err)?;
    let client = handle.lock().await;
    client.mock_url(&req.url_pattern, req.status, &req.body, &req.content_type)
        .await.map_err(cdp_err)?;
    Ok(Json(serde_json::json!({ "ok": true, "rule": "mock", "pattern": req.url_pattern })))
}

async fn browser_clear_intercepts(
    State(state): State<ApiState>,
    AxumPath(id): AxumPath<String>,
) -> ApiResult<Json<serde_json::Value>> {
    let cdp_port = require_cdp_port(&state, &id)?;
    let handle = state.session_manager.get_client(&id, cdp_port).await.map_err(cdp_err)?;
    let client = handle.lock().await;
    client.clear_intercepts().await.map_err(cdp_err)?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

async fn browser_print_to_pdf(
    State(state): State<ApiState>,
    AxumPath(id): AxumPath<String>,
    axum::extract::Query(req): axum::extract::Query<PdfReq>,
) -> ApiResult<Json<serde_json::Value>> {
    let cdp_port = require_cdp_port(&state, &id)?;
    let handle = state.session_manager.get_client(&id, cdp_port).await.map_err(cdp_err)?;
    let client = handle.lock().await;
    let data = client.print_to_pdf(req.landscape, req.print_background, req.scale)
        .await.map_err(cdp_err)?;
    Ok(Json(serde_json::json!({ "data": data, "format": "pdf" })))
}

async fn browser_tap(
    State(state): State<ApiState>,
    AxumPath(id): AxumPath<String>,
    Json(req): Json<TapReq>,
) -> ApiResult<Json<serde_json::Value>> {
    let cdp_port = require_cdp_port(&state, &id)?;
    let handle = state.session_manager.get_client(&id, cdp_port).await.map_err(cdp_err)?;
    let client = handle.lock().await;
    client.tap(&req.selector).await.map_err(cdp_err)?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

async fn browser_swipe(
    State(state): State<ApiState>,
    AxumPath(id): AxumPath<String>,
    Json(req): Json<SwipeReq>,
) -> ApiResult<Json<serde_json::Value>> {
    let cdp_port = require_cdp_port(&state, &id)?;
    let handle = state.session_manager.get_client(&id, cdp_port).await.map_err(cdp_err)?;
    let client = handle.lock().await;
    client.swipe(&req.selector, &req.direction, req.distance)
        .await.map_err(cdp_err)?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

async fn browser_get_frames(
    State(state): State<ApiState>,
    AxumPath(id): AxumPath<String>,
) -> ApiResult<Json<serde_json::Value>> {
    let cdp_port = require_cdp_port(&state, &id)?;
    let handle = state.session_manager.get_client(&id, cdp_port).await.map_err(cdp_err)?;
    let client = handle.lock().await;
    let frames = client.get_frames().await.map_err(cdp_err)?;
    let value = serde_json::to_value(frames)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(value))
}

async fn browser_switch_frame(
    State(state): State<ApiState>,
    AxumPath(id): AxumPath<String>,
    Json(req): Json<SwitchFrameReq>,
) -> ApiResult<Json<serde_json::Value>> {
    let cdp_port = require_cdp_port(&state, &id)?;
    let handle = state.session_manager.get_client(&id, cdp_port).await.map_err(cdp_err)?;
    let client = handle.lock().await;
    client.switch_frame(&req.frame_id).await.map_err(cdp_err)?;
    Ok(Json(serde_json::json!({ "ok": true, "frame_id": req.frame_id })))
}

async fn browser_main_frame(
    State(state): State<ApiState>,
    AxumPath(id): AxumPath<String>,
) -> ApiResult<Json<serde_json::Value>> {
    let cdp_port = require_cdp_port(&state, &id)?;
    let handle = state.session_manager.get_client(&id, cdp_port).await.map_err(cdp_err)?;
    let client = handle.lock().await;
    client.main_frame().await;
    Ok(Json(serde_json::json!({ "ok": true })))
}

async fn browser_clear_console(
    State(state): State<ApiState>,
    AxumPath(id): AxumPath<String>,
) -> ApiResult<Json<serde_json::Value>> {
    let cdp_port = require_cdp_port(&state, &id)?;
    let handle = state.session_manager.get_client(&id, cdp_port).await.map_err(cdp_err)?;
    let client = handle.lock().await;
    client.clear_console_logs().await;
    Ok(Json(serde_json::json!({ "ok": true })))
}

// ---------------------------------------------------------------------------
// Action Log endpoints
// ---------------------------------------------------------------------------

#[derive(serde::Deserialize)]
struct ActionLogQuery {
    profile_id: Option<String>,
    #[serde(default = "default_log_limit")]
    limit: usize,
}

fn default_log_limit() -> usize {
    100
}

async fn get_action_log(
    State(state): State<ApiState>,
    Query(q): Query<ActionLogQuery>,
) -> Json<Vec<action_log::ActionEntry>> {
    let entries = state
        .action_log
        .get_filtered(q.profile_id.as_deref(), q.limit);
    Json(entries)
}

async fn clear_action_log(
    State(state): State<ApiState>,
    Query(q): Query<ActionLogQuery>,
) -> StatusCode {
    state.action_log.clear(q.profile_id.as_deref());
    StatusCode::NO_CONTENT
}

// ---------------------------------------------------------------------------
// Snapshot endpoints
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

// ---------------------------------------------------------------------------
// Cookie export / import
// ---------------------------------------------------------------------------

#[derive(serde::Deserialize)]
struct CookieExportQuery {
    #[serde(default = "default_json_format")]
    format: String,
}
fn default_json_format() -> String {
    "json".to_string()
}

async fn browser_export_cookies(
    State(state): State<ApiState>,
    AxumPath(id): AxumPath<String>,
    Query(q): Query<CookieExportQuery>,
) -> Result<axum::response::Response, ApiError> {
    use axum::response::IntoResponse;

    let cdp_port = require_cdp_port(&state, &id)?;
    let handle = state
        .session_manager
        .get_client(&id, cdp_port)
        .await
        .map_err(cdp_err)?;
    let client = handle.lock().await;
    let cookies = client.get_cookies().await.map_err(cdp_err)?;

    if q.format == "netscape" {
        let body = export_cookies_netscape(&cookies);
        Ok((
            [
                (axum::http::header::CONTENT_TYPE, "text/plain"),
                (
                    axum::http::header::CONTENT_DISPOSITION,
                    "attachment; filename=\"cookies.txt\"",
                ),
            ],
            body,
        )
            .into_response())
    } else {
        let json = serde_json::to_string_pretty(&cookies)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        Ok((
            [
                (axum::http::header::CONTENT_TYPE, "application/json"),
                (
                    axum::http::header::CONTENT_DISPOSITION,
                    "attachment; filename=\"cookies.json\"",
                ),
            ],
            json,
        )
            .into_response())
    }
}

#[derive(serde::Deserialize)]
struct CookieImportReq {
    format: String,
    data: String,
}

async fn browser_import_cookies(
    State(state): State<ApiState>,
    AxumPath(id): AxumPath<String>,
    Json(req): Json<CookieImportReq>,
) -> ApiResult<Json<serde_json::Value>> {
    let cdp_port = require_cdp_port(&state, &id)?;
    let handle = state
        .session_manager
        .get_client(&id, cdp_port)
        .await
        .map_err(cdp_err)?;
    let client = handle.lock().await;

    let cookies: Vec<crate::agent::types::CookieInfo> = if req.format == "netscape" {
        parse_cookies_netscape(&req.data)
    } else {
        serde_json::from_str(&req.data)
            .map_err(|e| (StatusCode::BAD_REQUEST, format!("Invalid JSON: {}", e)))?
    };

    let mut imported = 0usize;
    let mut errors: Vec<String> = Vec::new();
    for cookie in &cookies {
        match client.set_cookie_full(cookie).await {
            Ok(()) => imported += 1,
            Err(e) => errors.push(e),
        }
    }

    Ok(Json(serde_json::json!({
        "imported": imported,
        "errors": errors,
    })))
}

fn export_cookies_netscape(cookies: &[crate::agent::types::CookieInfo]) -> String {
    let mut out = String::from("# Netscape HTTP Cookie File\n");
    for c in cookies {
        let flag = if c.domain.starts_with('.') { "TRUE" } else { "FALSE" };
        let secure = if c.secure { "TRUE" } else { "FALSE" };
        let expires = c.expires as u64;
        out.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
            c.domain, flag, c.path, secure, expires, c.name, c.value
        ));
    }
    out
}

fn parse_cookies_netscape(data: &str) -> Vec<crate::agent::types::CookieInfo> {
    let mut cookies = Vec::new();
    for line in data.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() < 7 {
            continue;
        }
        let domain = parts[0].to_string();
        let path = parts[2].to_string();
        let secure = parts[3].eq_ignore_ascii_case("TRUE");
        let expires: f64 = parts[4].parse().unwrap_or(0.0);
        let name = parts[5].to_string();
        let value = parts[6].to_string();
        cookies.push(crate::agent::types::CookieInfo {
            name,
            value,
            domain,
            path,
            secure,
            http_only: false,
            expires,
        });
    }
    cookies
}

// ---------------------------------------------------------------------------
// Server
// ---------------------------------------------------------------------------

/// Build the full API app (router + optional API key auth + CORS).
/// Used by run_server and by integration tests to exercise API key middleware.
pub fn app(state: ApiState, api_key: Option<String>) -> Router {
    use tower::limit::ConcurrencyLimitLayer;
    let state_for_log = state.clone();
    let base_router = router(state);
    if let Some(key) = api_key {
        base_router
            .route_layer(middleware::from_fn_with_state(key, api_key_auth))
    } else {
        base_router
    }
    .layer(middleware::from_fn_with_state(state_for_log, action_log_middleware))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recording::RecordedActionType;

    // -----------------------------------------------------------------------
    // parse_path_for_log tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_browser_tool_path() {
        let (profile_id, tool) = parse_path_for_log("/api/browser/prof-123/navigate");
        assert_eq!(profile_id, "prof-123");
        assert_eq!(tool, "navigate");
    }

    #[test]
    fn test_parse_browser_nested_tool_path() {
        let (profile_id, tool) = parse_path_for_log("/api/browser/prof-abc/tabs/new");
        assert_eq!(profile_id, "prof-abc");
        assert_eq!(tool, "tabs/new");
    }

    #[test]
    fn test_parse_launch_path() {
        let (profile_id, tool) = parse_path_for_log("/api/launch/my-profile");
        assert_eq!(profile_id, "my-profile");
        assert_eq!(tool, "launch");
    }

    #[test]
    fn test_parse_kill_path() {
        let (profile_id, tool) = parse_path_for_log("/api/kill/my-profile");
        assert_eq!(profile_id, "my-profile");
        assert_eq!(tool, "kill");
    }

    #[test]
    fn test_parse_profiles_crud_path() {
        let (profile_id, tool) = parse_path_for_log("/api/profiles/some-id");
        assert_eq!(profile_id, "");
        assert_eq!(tool, "profiles/some-id");
    }

    #[test]
    fn test_parse_health_path() {
        let (profile_id, tool) = parse_path_for_log("/api/health");
        assert_eq!(profile_id, "");
        assert_eq!(tool, "health");
    }

    // -----------------------------------------------------------------------
    // tool_to_recorded_action tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_tool_navigate() {
        assert_eq!(tool_to_recorded_action("navigate"), Some(RecordedActionType::Navigate));
    }

    #[test]
    fn test_tool_navigate_wait() {
        assert_eq!(tool_to_recorded_action("navigate_wait"), Some(RecordedActionType::Navigate));
    }

    #[test]
    fn test_tool_click() {
        assert_eq!(tool_to_recorded_action("click"), Some(RecordedActionType::Click));
    }

    #[test]
    fn test_tool_screenshot() {
        assert_eq!(tool_to_recorded_action("screenshot"), Some(RecordedActionType::Screenshot));
    }

    #[test]
    fn test_tool_new_tab() {
        assert_eq!(tool_to_recorded_action("new_tab"), Some(RecordedActionType::NewTab));
    }

    #[test]
    fn test_tool_unknown() {
        assert_eq!(tool_to_recorded_action("some_unknown_tool"), None);
    }

    #[test]
    fn test_tool_empty_string() {
        assert_eq!(tool_to_recorded_action(""), None);
    }

    #[test]
    fn test_api_ok_object_preserves_fields_and_adds_ok() {
        let response = api_ok(serde_json::json!({ "url": "http://example.com", "title": "Example" }));
        let value = response.0;
        assert_eq!(value.get("ok").and_then(|v| v.as_bool()), Some(true));
        assert_eq!(value.get("url").and_then(|v| v.as_str()), Some("http://example.com"));
        assert_eq!(value.get("title").and_then(|v| v.as_str()), Some("Example"));
    }

    #[test]
    fn test_api_error_into_response_returns_json_envelope() {
        let response = ApiError::new(StatusCode::CONFLICT, "browser_not_running", "Profile test is not running")
            .with_detail("profile_id", serde_json::json!("test"))
            .into_response();
        assert_eq!(response.status(), StatusCode::CONFLICT);
    }
}
