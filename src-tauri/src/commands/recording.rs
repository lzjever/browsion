//! Tauri commands for recording management.

use crate::recording::{RecordedAction, RecordedActionType, Recording};
use crate::state::AppState;
use std::collections::HashMap;
use std::sync::Arc;

fn merge_action_params(
    left: &serde_json::Value,
    right: &serde_json::Value,
) -> serde_json::Value {
    let mut merged = left.clone();
    if let (Some(target), Some(source)) = (merged.as_object_mut(), right.as_object()) {
        for (key, value) in source {
            target.entry(key.clone()).or_insert_with(|| value.clone());
        }
    }
    merged
}

fn should_merge_adjacent_actions(previous: &RecordedAction, action: &RecordedAction) -> bool {
    let close_in_time = action.timestamp_ms.saturating_sub(previous.timestamp_ms) <= 100;
    if !close_in_time {
        return false;
    }

    match (&previous.action_type, &action.action_type) {
        (RecordedActionType::Click, RecordedActionType::Click)
        | (RecordedActionType::DoubleClick, RecordedActionType::DoubleClick)
        | (RecordedActionType::RightClick, RecordedActionType::RightClick) => {
            previous.params.get("selector") == action.params.get("selector")
        }
        _ => false,
    }
}

pub(crate) fn normalize_recording_actions(actions: Vec<RecordedAction>) -> Vec<RecordedAction> {
    let has_non_tab_action = actions.iter().any(|action| {
        !matches!(
            action.action_type,
            RecordedActionType::NewTab | RecordedActionType::SwitchTab | RecordedActionType::CloseTab
        )
    });
    let has_explicit_navigate = actions
        .iter()
        .any(|action| action.action_type == RecordedActionType::Navigate);

    let mut normalized: Vec<RecordedAction> = Vec::new();

    for mut action in actions {
        match action.action_type {
            RecordedActionType::Type => {
                if action.params.get("text").is_none() {
                    if let Some(text) = action.params.get("value").cloned() {
                        action.params["text"] = text;
                    }
                }

                if let Some(previous) = normalized.last_mut() {
                    let same_selector = previous.action_type == RecordedActionType::Type
                        && previous.params.get("selector") == action.params.get("selector");
                    let close_in_time = action.timestamp_ms.saturating_sub(previous.timestamp_ms) <= 250;
                    if same_selector && close_in_time {
                        *previous = action;
                        continue;
                    }
                }
            }
            RecordedActionType::SelectOption => {}
            RecordedActionType::NewTab => {
                if normalized.is_empty() && has_non_tab_action {
                    if has_explicit_navigate {
                        continue;
                    }
                    let url = action.params.get("url").and_then(|v| v.as_str()).unwrap_or("");
                    if !url.is_empty() && !url.starts_with("chrome://") {
                        action.action_type = RecordedActionType::Navigate;
                        action.params = serde_json::json!({ "url": url });
                    } else {
                        continue;
                    }
                }
            }
            RecordedActionType::CloseTab => {
                if normalized.is_empty() && has_non_tab_action {
                    continue;
                }
            }
            _ => {}
        }

        if let Some(previous) = normalized.last() {
            let same_moment = previous.timestamp_ms == action.timestamp_ms;
            let same_type = previous.action_type == action.action_type;
            let same_params = previous.params == action.params;
            if same_moment && same_type && same_params {
                continue;
            }

            if should_merge_adjacent_actions(previous, &action) {
                let previous = normalized.last_mut().unwrap();
                previous.params = merge_action_params(&previous.params, &action.params);
                previous.timestamp_ms = previous.timestamp_ms.min(action.timestamp_ms);
                continue;
            }

            let duplicate_switch_target = action.action_type == RecordedActionType::SwitchTab
                && previous.action_type == RecordedActionType::SwitchTab
                && previous.params.get("target_id") == action.params.get("target_id");
            if duplicate_switch_target {
                *normalized.last_mut().unwrap() = action;
                continue;
            }
        }

        normalized.push(action);
    }

    for (index, action) in normalized.iter_mut().enumerate() {
        action.index = index;
    }

    normalized
}

pub(crate) fn map_manual_event_to_action(
    event_type: &str,
    data: &serde_json::Value,
) -> Option<(RecordedActionType, serde_json::Value)> {
    match event_type {
        "click" => Some((RecordedActionType::Click, data.clone())),
        "input" => Some((
            RecordedActionType::Type,
            serde_json::json!({
                "selector": data.get("selector").cloned().unwrap_or(serde_json::Value::Null),
                "text": data
                    .get("text")
                    .cloned()
                    .or_else(|| data.get("value").cloned())
                    .unwrap_or_else(|| serde_json::json!("")),
            }),
        )),
        "change" => {
            let tag_name = data.get("tag_name").and_then(|v| v.as_str()).unwrap_or("");
            let input_type = data.get("input_type").and_then(|v| v.as_str()).unwrap_or("");

            if tag_name == "select" {
                Some((
                    RecordedActionType::SelectOption,
                    serde_json::json!({
                        "selector": data.get("selector").cloned().unwrap_or(serde_json::Value::Null),
                        "value": data.get("value").cloned().unwrap_or(serde_json::Value::Null),
                    }),
                ))
            } else if matches!(input_type, "checkbox" | "radio") {
                None
            } else {
                None
            }
        }
        "keydown" => Some((
            RecordedActionType::PressKey,
            serde_json::json!({
                "key": data.get("key").cloned().unwrap_or_else(|| serde_json::json!("")),
            }),
        )),
        _ => None,
    }
}

#[tauri::command]
pub async fn list_recordings(state: tauri::State<'_, Arc<AppState>>) -> Result<Vec<Recording>, String> {
    Ok(state.recording_manager.list())
}

#[tauri::command]
pub async fn get_recording(
    id: String,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<Recording, String> {
    state
        .recording_manager
        .get(&id)
        .ok_or_else(|| "Recording not found".to_string())
}

#[tauri::command]
pub async fn save_recording(
    recording: Recording,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<Recording, String> {
    state
        .recording_manager
        .save(recording.clone())
        .map_err(|e| e.to_string())?;
    Ok(recording)
}

#[tauri::command]
pub async fn delete_recording(
    id: String,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<(), String> {
    state
        .recording_manager
        .delete(&id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn play_recording(
    recording_id: String,
    profile_id: String,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<crate::recording::PlaybackResult, String> {
    let recording = state
        .recording_manager
        .get(&recording_id)
        .ok_or_else(|| "Recording not found".to_string())?;
    crate::recording::playback::play_recording_on_profile(
        Arc::clone(state.inner()),
        recording,
        profile_id,
    )
    .await
}

/// Real-time recording commands
/// Start recording for a profile.
#[tauri::command]
pub async fn start_recording(
    profile_id: String,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<String, String> {
    tracing::info!("Starting recording for profile {}", profile_id);

    let session_id = state
        .recording_session_manager
        .start_session(profile_id.clone())?;

    // Start manual recording by injecting JS listeners
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

    Ok(session_id)
}

/// Stop recording for a profile.
#[tauri::command]
pub async fn stop_recording(
    profile_id: String,
    name: String,
    description: String,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<Recording, String> {
    tracing::info!("Stopping recording for profile {}", profile_id);

    // IMPORTANT: Re-inject listeners before stopping to ensure we capture
    // any events from the CURRENT active tab (user might have navigated)
    if let Some(cdp_port) = state.process_manager.get_cdp_port(&profile_id) {
        tracing::info!("Re-injecting listeners to capture final events...");
        match state.session_manager.get_client(&profile_id, cdp_port).await {
            Ok(handle) => {
                let client = handle.lock().await;
                let _ = client.start_manual_recording().await;
                // Give a small delay for any immediate events to be captured
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            }
            Err(e) => {
                tracing::error!("Failed to re-inject listeners: {}", e);
            }
        }
    }

    // Extract manual recording events from console log before stopping
    if let Some(cdp_port) = state.process_manager.get_cdp_port(&profile_id) {
        tracing::info!("Extracting events from console log (CDP port: {})", cdp_port);
        match state.session_manager.get_client(&profile_id, cdp_port).await {
            Ok(handle) => {
                let client = handle.lock().await;

                // First, extract manual recording events from console log
                let console_log = client.get_console_log().await;
                tracing::info!("Console log has {} entries", console_log.len());

                let mut browsion_event_count = 0;
                for entry in &console_log {
                    if let Some(args) = entry.get("args").and_then(|a| a.as_array()) {
                        if args.len() >= 2 && args[0] == "__BROWSION_EVENT__" {
                            browsion_event_count += 1;
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
                                                Ok(_) => {}
                                                Err(e) => tracing::error!("Failed to add action: {}", e),
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                tracing::info!("Found {} manual recording events in console log", browsion_event_count);

                // Now stop the recording (this will send __BROWSION_STOPPED__)
                let _ = client.stop_manual_recording().await;
            }
            Err(e) => {
                tracing::error!("Failed to get CDP client: {}", e);
            }
        }
    } else {
        tracing::warn!("No CDP port found for profile {}", profile_id);
    }

    let mut recording = state
        .recording_session_manager
        .stop_session(&profile_id)?;

    // Update metadata
    recording.name = name;
    recording.description = description;
    recording.actions = normalize_recording_actions(recording.actions);

    // Save recording
    state
        .recording_manager
        .save(recording.clone())
        .map_err(|e| e.to_string())?;

    // Emit event for UI update
    state.emit("recording-status-changed");

    tracing::info!("Recording saved: {} with {} actions", recording.name, recording.actions.len());

    Ok(recording)
}

/// Get active recording sessions.
#[tauri::command]
pub async fn get_active_recording_sessions(
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<HashMap<String, String>, String> {
    Ok(state.recording_session_manager.get_active_sessions())
}

/// Check if a profile is currently being recorded.
#[tauri::command]
pub async fn is_recording(
    profile_id: String,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<bool, String> {
    Ok(state.recording_session_manager.is_recording(&profile_id))
}

/// Get session info for a profile.
#[tauri::command]
pub async fn get_recording_session_info(
    profile_id: String,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<Option<crate::recording::RecordingSessionInfo>, String> {
    Ok(state.recording_session_manager.get_session_info(&profile_id))
}

#[cfg(test)]
mod tests {
    use super::normalize_recording_actions;
    use crate::recording::{RecordedAction, RecordedActionType};

    fn action(
        index: usize,
        action_type: RecordedActionType,
        params: serde_json::Value,
        timestamp_ms: u64,
    ) -> RecordedAction {
        RecordedAction {
            index,
            action_type,
            params,
            timestamp_ms,
            screenshot_base64: None,
        }
    }

    #[test]
    fn test_normalize_dedupes_identical_actions_at_same_timestamp() {
        let normalized = normalize_recording_actions(vec![
            action(7, RecordedActionType::Click, serde_json::json!({"selector":"#btn","x":1,"y":2}), 1000),
            action(8, RecordedActionType::Click, serde_json::json!({"selector":"#btn","x":1,"y":2}), 1000),
            action(9, RecordedActionType::Type, serde_json::json!({"selector":"#name","text":"A"}), 1100),
        ]);

        assert_eq!(normalized.len(), 2);
        assert_eq!(normalized[0].action_type, RecordedActionType::Click);
        assert_eq!(normalized[0].index, 0);
        assert_eq!(normalized[1].action_type, RecordedActionType::Type);
        assert_eq!(normalized[1].index, 1);
    }

    #[test]
    fn test_normalize_reindexes_actions_sequentially() {
        let normalized = normalize_recording_actions(vec![
            action(3, RecordedActionType::Navigate, serde_json::json!({"url":"https://example.com"}), 100),
            action(9, RecordedActionType::Click, serde_json::json!({"selector":"#btn"}), 200),
        ]);

        assert_eq!(normalized.len(), 2);
        assert_eq!(normalized[0].index, 0);
        assert_eq!(normalized[1].index, 1);
    }

    #[test]
    fn test_normalize_drops_leading_tab_noise_before_real_actions() {
        let normalized = normalize_recording_actions(vec![
            action(0, RecordedActionType::CloseTab, serde_json::json!({"target_id":"tab-z"}), 120),
            action(1, RecordedActionType::NewTab, serde_json::json!({"target_id":"tab-c","url":"chrome://newtab/"}), 140),
            action(2, RecordedActionType::Click, serde_json::json!({"selector":"#btn"}), 400),
        ]);

        assert_eq!(normalized.len(), 1);
        assert_eq!(normalized[0].action_type, RecordedActionType::Click);
        assert_eq!(normalized[0].index, 0);
    }

    #[test]
    fn test_normalize_drops_leading_new_tab_prefix_when_explicit_navigate_exists() {
        let normalized = normalize_recording_actions(vec![
            action(0, RecordedActionType::NewTab, serde_json::json!({"target_id":"tab-a","url":"https://noise.example"}), 100),
            action(1, RecordedActionType::NewTab, serde_json::json!({"target_id":"tab-b","url":"https://noise-2.example"}), 110),
            action(2, RecordedActionType::Navigate, serde_json::json!({"url":"https://real.example"}), 300),
            action(3, RecordedActionType::Click, serde_json::json!({"selector":"#btn"}), 400),
        ]);

        assert_eq!(normalized.len(), 2);
        assert_eq!(normalized[0].action_type, RecordedActionType::Navigate);
        assert_eq!(normalized[0].params.get("url").and_then(|v| v.as_str()), Some("https://real.example"));
        assert_eq!(normalized[1].action_type, RecordedActionType::Click);
    }

    #[test]
    fn test_normalize_merges_redundant_switch_tab_to_same_target() {
        let normalized = normalize_recording_actions(vec![
            action(0, RecordedActionType::Navigate, serde_json::json!({"url":"https://example.com"}), 100),
            action(1, RecordedActionType::SwitchTab, serde_json::json!({"target_id":"tab-b","previous_target_id":"tab-a"}), 200),
            action(2, RecordedActionType::SwitchTab, serde_json::json!({"target_id":"tab-b","previous_target_id":"tab-a"}), 210),
            action(3, RecordedActionType::Click, serde_json::json!({"selector":"#btn"}), 300),
        ]);

        assert_eq!(normalized.len(), 3);
        assert_eq!(normalized[1].action_type, RecordedActionType::SwitchTab);
        assert_eq!(normalized[1].timestamp_ms, 210);
        assert_eq!(normalized[1].index, 1);
    }

    #[test]
    fn test_normalize_merges_duplicate_clicks_on_same_selector() {
        let normalized = normalize_recording_actions(vec![
            action(0, RecordedActionType::Click, serde_json::json!({"selector":"#btn"}), 1000),
            action(1, RecordedActionType::Click, serde_json::json!({"selector":"#btn","x":53,"y":124}), 1001),
        ]);

        assert_eq!(normalized.len(), 1);
        assert_eq!(normalized[0].action_type, RecordedActionType::Click);
        assert_eq!(normalized[0].params["selector"], serde_json::json!("#btn"));
        assert_eq!(normalized[0].params["x"], serde_json::json!(53));
        assert_eq!(normalized[0].params["y"], serde_json::json!(124));
        assert_eq!(normalized[0].index, 0);
    }
}
