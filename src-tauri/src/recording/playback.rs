use crate::commands::get_effective_chrome_path_from_config;
use crate::recording::{RecordedAction, RecordedActionType, Recording};
use crate::state::AppState;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize)]
pub struct PlaybackResult {
    pub recording_id: String,
    pub profile_id: String,
    pub completed_actions: usize,
    pub total_actions: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct PlaybackProgressEvent {
    pub recording_id: String,
    pub profile_id: String,
    pub action_index: usize,
    pub total_actions: usize,
    pub action_type: String,
    pub status: &'static str,
    pub error: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct PlaybackTab {
    id: String,
    #[allow(dead_code)]
    url: String,
    #[allow(dead_code)]
    title: String,
    #[allow(dead_code)]
    r#type: String,
    active: Option<bool>,
}

#[derive(Debug, Default)]
struct PlaybackContext {
    recorded_to_runtime_tab_id: HashMap<String, String>,
    current_runtime_tab_id: Option<String>,
    pending_previous_runtime_tab_id: Option<String>,
}

pub async fn play_recording_on_profile(
    state: Arc<AppState>,
    recording: Recording,
    profile_id: String,
) -> Result<PlaybackResult, String> {
    ensure_profile_running(&state, &profile_id).await?;

    let config = state.config.read().clone();
    if !config.mcp.enabled {
        return Err("Local API is disabled".to_string());
    }

    let base = format!("http://127.0.0.1:{}", config.mcp.api_port);
    let client = reqwest::Client::new();
    let api_key = config.mcp.api_key.clone();
    let mut context = create_playback_context(&client, &base, api_key.as_deref(), &profile_id).await?;

    let mut completed = 0usize;
    let mut index = 0usize;
    while index < recording.actions.len() {
        let action = &recording.actions[index];
        let next_action = recording.actions.get(index + 1);
        state.emit_payload(
            "recording-playback-progress",
            PlaybackProgressEvent {
                recording_id: recording.id.clone(),
                profile_id: profile_id.clone(),
                action_index: index,
                total_actions: recording.actions.len(),
                action_type: serde_json::to_string(&action.action_type)
                    .unwrap_or_else(|_| "\"unknown\"".to_string())
                    .trim_matches('"')
                    .to_string(),
                status: "running",
                error: None,
            },
        );
        state.broadcast_ws(crate::api::ws::WsEvent::RecordingPlaybackProgress {
            recording_id: recording.id.clone(),
            profile_id: profile_id.clone(),
            action_index: index,
            total_actions: recording.actions.len(),
            action_type: serde_json::to_string(&action.action_type)
                .unwrap_or_else(|_| "\"unknown\"".to_string())
                .trim_matches('"')
                .to_string(),
            status: "running".to_string(),
            error: None,
        });

        let popup_action = if should_fold_popup_into_action(action, next_action) {
            next_action.cloned()
        } else {
            None
        };

        execute_action(
            &client,
            &base,
            api_key.as_deref(),
            &profile_id,
            &mut context,
            action,
            popup_action.as_ref(),
        )
        .await
        .map_err(|err| {
            state.emit_payload(
                "recording-playback-progress",
                PlaybackProgressEvent {
                    recording_id: recording.id.clone(),
                    profile_id: profile_id.clone(),
                    action_index: index,
                    total_actions: recording.actions.len(),
                    action_type: serde_json::to_string(&action.action_type)
                        .unwrap_or_else(|_| "\"unknown\"".to_string())
                        .trim_matches('"')
                        .to_string(),
                    status: "failed",
                    error: Some(err.clone()),
                },
            );
            state.broadcast_ws(crate::api::ws::WsEvent::RecordingPlaybackProgress {
                recording_id: recording.id.clone(),
                profile_id: profile_id.clone(),
                action_index: index,
                total_actions: recording.actions.len(),
                action_type: serde_json::to_string(&action.action_type)
                    .unwrap_or_else(|_| "\"unknown\"".to_string())
                    .trim_matches('"')
                    .to_string(),
                status: "failed".to_string(),
                error: Some(err.clone()),
            });
            format!("Action {} failed: {}", action.index + 1, err)
        })?;

        completed += 1;
        if popup_action.is_some() {
            completed += 1;
            index += 2;
        } else {
            index += 1;
        }
    }

    state.emit_payload(
        "recording-playback-progress",
        PlaybackProgressEvent {
            recording_id: recording.id.clone(),
            profile_id: profile_id.clone(),
            action_index: recording.actions.len(),
            total_actions: recording.actions.len(),
            action_type: "complete".to_string(),
            status: "completed",
            error: None,
        },
    );
    state.broadcast_ws(crate::api::ws::WsEvent::RecordingPlaybackProgress {
        recording_id: recording.id.clone(),
        profile_id: profile_id.clone(),
        action_index: recording.actions.len(),
        total_actions: recording.actions.len(),
        action_type: "complete".to_string(),
        status: "completed".to_string(),
        error: None,
    });

    Ok(PlaybackResult {
        recording_id: recording.id,
        profile_id,
        completed_actions: completed,
        total_actions: recording.actions.len(),
    })
}

async fn ensure_profile_running(state: &Arc<AppState>, profile_id: &str) -> Result<(), String> {
    if state.process_manager.is_running(profile_id) {
        return Ok(());
    }

    let config = state.config.read().clone();
    let chrome_path = get_effective_chrome_path_from_config(&config).await?;
    state
        .process_manager
        .launch_profile(profile_id, &config, &chrome_path)
        .await
        .map_err(|e| e.to_string())?;

    tokio::time::sleep(tokio::time::Duration::from_millis(1500)).await;
    Ok(())
}

async fn create_playback_context(
    client: &reqwest::Client,
    base: &str,
    api_key: Option<&str>,
    profile_id: &str,
) -> Result<PlaybackContext, String> {
    let tabs = list_tabs(client, base, api_key, profile_id).await?;
    let active_tab = tabs
        .iter()
        .find(|tab| tab.active.unwrap_or(false))
        .or_else(|| tabs.first());

    Ok(PlaybackContext {
        recorded_to_runtime_tab_id: HashMap::new(),
        current_runtime_tab_id: active_tab.map(|tab| tab.id.clone()),
        pending_previous_runtime_tab_id: None,
    })
}

async fn list_tabs(
    client: &reqwest::Client,
    base: &str,
    api_key: Option<&str>,
    profile_id: &str,
) -> Result<Vec<PlaybackTab>, String> {
    let mut request = client.get(format!("{base}/api/browser/{profile_id}/tabs"));
    if let Some(api_key) = api_key {
        request = request.header("X-API-Key", api_key);
    }

    let response = request.send().await.map_err(|e| e.to_string())?;
    let status = response.status();
    if !status.is_success() {
        return Err(format!("HTTP {}", status.as_u16()));
    }

    response.json().await.map_err(|e| e.to_string())
}

async fn refresh_active_tab(
    client: &reqwest::Client,
    base: &str,
    api_key: Option<&str>,
    profile_id: &str,
    context: &mut PlaybackContext,
) -> Result<(), String> {
    let tabs = list_tabs(client, base, api_key, profile_id).await?;
    let active_tab = tabs
        .iter()
        .find(|tab| tab.active.unwrap_or(false))
        .or_else(|| tabs.first());
    context.current_runtime_tab_id = active_tab.map(|tab| tab.id.clone());
    Ok(())
}

async fn wait_for_popup_from_action(
    client: &reqwest::Client,
    base: &str,
    api_key: Option<&str>,
    profile_id: &str,
    context: &mut PlaybackContext,
    previous_tabs: &[PlaybackTab],
    recorded_popup_action: &RecordedAction,
) -> Result<(), String> {
    let previous_ids: HashSet<String> = previous_tabs.iter().map(|tab| tab.id.clone()).collect();
    let previous_active_runtime_tab_id = context.current_runtime_tab_id.clone();

    for _ in 0..20 {
        tokio::time::sleep(tokio::time::Duration::from_millis(250)).await;
        let tabs = list_tabs(client, base, api_key, profile_id).await?;
        if let Some(popup) = tabs.iter().find(|tab| !previous_ids.contains(&tab.id)) {
            if let Some(recorded_target_id) = recorded_popup_action
                .params
                .get("target_id")
                .and_then(|value| value.as_str())
            {
                context
                    .recorded_to_runtime_tab_id
                    .insert(recorded_target_id.to_string(), popup.id.clone());
            }
            context.pending_previous_runtime_tab_id = previous_active_runtime_tab_id.clone();
            context.current_runtime_tab_id = Some(popup.id.clone());
            return Ok(());
        }
    }

    Err("Expected the previous action to open a new tab, but no new tab appeared".to_string())
}

fn should_fold_popup_into_action(
    action: &RecordedAction,
    next_action: Option<&RecordedAction>,
) -> bool {
    matches!(next_action, Some(next) if next.action_type == RecordedActionType::NewTab)
        && matches!(
            action.action_type,
            RecordedActionType::Click
                | RecordedActionType::DoubleClick
                | RecordedActionType::RightClick
                | RecordedActionType::PressKey
        )
}

fn resolve_runtime_tab_id(
    recorded_target_id: &serde_json::Value,
    context: &PlaybackContext,
) -> Result<String, String> {
    let recorded_target_id = recorded_target_id
        .as_str()
        .ok_or_else(|| "Missing target_id for tab action".to_string())?;
    Ok(context
        .recorded_to_runtime_tab_id
        .get(recorded_target_id)
        .cloned()
        .unwrap_or_else(|| recorded_target_id.to_string()))
}

async fn execute_action(
    client: &reqwest::Client,
    base: &str,
    api_key: Option<&str>,
    profile_id: &str,
    context: &mut PlaybackContext,
    action: &RecordedAction,
    popup_action: Option<&RecordedAction>,
) -> Result<(), String> {
    if action.action_type == RecordedActionType::Sleep {
        let duration = action
            .params
            .get("duration_ms")
            .and_then(|value| value.as_u64())
            .unwrap_or(1000);
        tokio::time::sleep(tokio::time::Duration::from_millis(duration)).await;
        return Ok(());
    }

    match action.action_type {
        RecordedActionType::Navigate => {
            http_post_json(
                client,
                base,
                api_key,
                format!("/api/browser/{profile_id}/navigate_wait"),
                merge_defaults(&action.params, serde_json::json!({
                    "wait_until": "load",
                    "timeout_ms": 30000
                })),
            )
            .await?;
        }
        RecordedActionType::GoBack => {
            http_post_json(client, base, api_key, format!("/api/browser/{profile_id}/back"), serde_json::json!({})).await?;
        }
        RecordedActionType::GoForward => {
            http_post_json(client, base, api_key, format!("/api/browser/{profile_id}/forward"), serde_json::json!({})).await?;
        }
        RecordedActionType::Reload => {
            http_post_json(client, base, api_key, format!("/api/browser/{profile_id}/reload"), serde_json::json!({})).await?;
        }
        RecordedActionType::Click
        | RecordedActionType::DoubleClick
        | RecordedActionType::RightClick
        | RecordedActionType::PressKey => {
            let before_tabs = if popup_action.is_some() {
                list_tabs(client, base, api_key, profile_id).await?
            } else {
                Vec::new()
            };

            let path = match action.action_type {
                RecordedActionType::Click => "/click",
                RecordedActionType::DoubleClick => "/double_click",
                RecordedActionType::RightClick => "/right_click",
                RecordedActionType::PressKey => "/press_key",
                _ => unreachable!(),
            };
            http_post_json(client, base, api_key, format!("/api/browser/{profile_id}{path}"), action.params.clone()).await?;

            if let Some(popup_action) = popup_action {
                wait_for_popup_from_action(
                    client,
                    base,
                    api_key,
                    profile_id,
                    context,
                    &before_tabs,
                    popup_action,
                )
                .await?;
            }
        }
        RecordedActionType::Hover => {
            http_post_json(client, base, api_key, format!("/api/browser/{profile_id}/hover"), action.params.clone()).await?;
        }
        RecordedActionType::Type => {
            http_post_json(
                client,
                base,
                api_key,
                format!("/api/browser/{profile_id}/type"),
                serde_json::json!({
                    "selector": action.params.get("selector").cloned().unwrap_or(serde_json::Value::Null),
                    "text": action.params.get("text").cloned().or_else(|| action.params.get("value").cloned()).unwrap_or_else(|| serde_json::json!("")),
                }),
            )
            .await?;
        }
        RecordedActionType::SlowType => {
            let mut body = action.params.clone();
            if body.get("text").is_none() {
                if let Some(text) = body.get("value").cloned() {
                    body["text"] = text;
                }
            }
            http_post_json(client, base, api_key, format!("/api/browser/{profile_id}/slow_type"), body).await?;
        }
        RecordedActionType::SelectOption => {
            http_post_json(client, base, api_key, format!("/api/browser/{profile_id}/select_option"), action.params.clone()).await?;
        }
        RecordedActionType::UploadFile => {
            http_post_json(client, base, api_key, format!("/api/browser/{profile_id}/upload_file"), action.params.clone()).await?;
        }
        RecordedActionType::Scroll => {
            http_post_json(client, base, api_key, format!("/api/browser/{profile_id}/scroll"), action.params.clone()).await?;
        }
        RecordedActionType::ScrollIntoView => {
            http_post_json(
                client,
                base,
                api_key,
                format!("/api/browser/{profile_id}/scroll_into_view"),
                action.params.clone(),
            )
            .await?;
        }
        RecordedActionType::NewTab => {
            context.pending_previous_runtime_tab_id = context.current_runtime_tab_id.clone();
            if action.index == 0 {
                if let Some(url) = action.params.get("url").and_then(|value| value.as_str()) {
                    if !url.is_empty() && !url.starts_with("chrome://") {
                        http_post_json(
                            client,
                            base,
                            api_key,
                            format!("/api/browser/{profile_id}/navigate_wait"),
                            serde_json::json!({
                                "url": url,
                                "wait_until": "load",
                                "timeout_ms": 30000
                            }),
                        )
                        .await?;
                        return Ok(());
                    }
                }
            }

            let created_tab = http_post_json(
                client,
                base,
                api_key,
                format!("/api/browser/{profile_id}/tabs/new"),
                serde_json::json!({
                    "url": action
                        .params
                        .get("url")
                        .and_then(|value| value.as_str())
                        .filter(|url| !url.is_empty())
                        .unwrap_or("about:blank")
                }),
            )
            .await?;
            if let (Some(recorded_target_id), Some(runtime_id)) = (
                action.params.get("target_id").and_then(|value| value.as_str()),
                created_tab.get("id").and_then(|value| value.as_str()),
            ) {
                context
                    .recorded_to_runtime_tab_id
                    .insert(recorded_target_id.to_string(), runtime_id.to_string());
                context.current_runtime_tab_id = Some(runtime_id.to_string());
            }
        }
        RecordedActionType::SwitchTab => {
            if let (Some(previous_recorded_target_id), Some(previous_runtime_tab_id)) = (
                action.params.get("previous_target_id").and_then(|value| value.as_str()),
                context
                    .pending_previous_runtime_tab_id
                    .take()
                    .or_else(|| context.current_runtime_tab_id.clone()),
            ) {
                context
                    .recorded_to_runtime_tab_id
                    .entry(previous_recorded_target_id.to_string())
                    .or_insert(previous_runtime_tab_id);
            }

            let target_id = resolve_runtime_tab_id(
                action.params.get("target_id").unwrap_or(&serde_json::Value::Null),
                context,
            )?;
            http_post_json(
                client,
                base,
                api_key,
                format!("/api/browser/{profile_id}/tabs/switch"),
                serde_json::json!({ "target_id": target_id }),
            )
            .await?;
            context.current_runtime_tab_id = Some(target_id);
        }
        RecordedActionType::CloseTab => {
            let target_id = resolve_runtime_tab_id(
                action.params.get("target_id").unwrap_or(&serde_json::Value::Null),
                context,
            )?;
            http_post_json(
                client,
                base,
                api_key,
                format!("/api/browser/{profile_id}/tabs/close"),
                serde_json::json!({ "target_id": target_id }),
            )
            .await?;
            refresh_active_tab(client, base, api_key, profile_id, context).await?;
        }
        RecordedActionType::WaitForText => {
            http_post_json(
                client,
                base,
                api_key,
                format!("/api/browser/{profile_id}/wait_for_text"),
                merge_defaults(&action.params, serde_json::json!({ "timeout_ms": 10000 })),
            )
            .await?;
        }
        RecordedActionType::WaitForElement => {
            http_post_json(
                client,
                base,
                api_key,
                format!("/api/browser/{profile_id}/wait_for"),
                merge_defaults(&action.params, serde_json::json!({ "timeout_ms": 10000 })),
            )
            .await?;
        }
        RecordedActionType::Screenshot => {
            http_get(client, base, api_key, format!("/api/browser/{profile_id}/screenshot")).await?;
        }
        RecordedActionType::GetConsoleLogs => {
            http_get(client, base, api_key, format!("/api/browser/{profile_id}/console")).await?;
        }
        RecordedActionType::Extract => {
            http_post_json(client, base, api_key, format!("/api/browser/{profile_id}/extract"), action.params.clone()).await?;
        }
        RecordedActionType::Sleep => unreachable!(),
    }

    Ok(())
}

fn merge_defaults(value: &serde_json::Value, defaults: serde_json::Value) -> serde_json::Value {
    let mut merged = defaults;
    if let (Some(target), Some(source)) = (merged.as_object_mut(), value.as_object()) {
        for (key, item) in source {
            target.insert(key.clone(), item.clone());
        }
    }
    merged
}

async fn http_post_json(
    client: &reqwest::Client,
    base: &str,
    api_key: Option<&str>,
    path: String,
    body: serde_json::Value,
) -> Result<serde_json::Value, String> {
    let mut request = client.post(format!("{base}{path}")).json(&body);
    if let Some(api_key) = api_key {
        request = request.header("X-API-Key", api_key);
    }
    let response = request.send().await.map_err(|e| e.to_string())?;
    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(format!("HTTP {} {}", status.as_u16(), body));
    }
    Ok(response.json().await.unwrap_or(serde_json::Value::Null))
}

async fn http_get(
    client: &reqwest::Client,
    base: &str,
    api_key: Option<&str>,
    path: String,
) -> Result<(), String> {
    let mut request = client.get(format!("{base}{path}"));
    if let Some(api_key) = api_key {
        request = request.header("X-API-Key", api_key);
    }
    let response = request.send().await.map_err(|e| e.to_string())?;
    let status = response.status();
    if status.is_success() {
        Ok(())
    } else {
        let body = response.text().await.unwrap_or_default();
        Err(format!("HTTP {} {}", status.as_u16(), body))
    }
}
