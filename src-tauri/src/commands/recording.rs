//! Tauri commands for recording management.

use crate::recording::Recording;
use crate::state::AppState;
use crate::workflow::{Workflow, WorkflowStep};
use std::collections::HashMap;
use std::sync::Arc;

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

/// Convert a recording to a workflow.
#[tauri::command]
pub async fn recording_to_workflow(
    recording_id: String,
    workflow_name: String,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<Workflow, String> {
    let recording = state
        .recording_manager
        .get(&recording_id)
        .ok_or_else(|| "Recording not found".to_string())?;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    let steps: Result<Vec<WorkflowStep>, String> = recording
        .actions
        .into_iter()
        .map(|action| {
            let step_type = crate::workflow::schema::StepType::from(action.action_type);
            Ok(WorkflowStep {
                id: format!("step-{}", action.index),
                name: format!("{} ({}ms)", step_type, action.timestamp_ms),
                description: String::new(),
                step_type,
                params: action.params,
                continue_on_error: false,
                timeout_ms: 30000,
            })
        })
        .collect();

    let workflow = Workflow {
        id: format!("workflow-from-{}", recording_id),
        name: workflow_name,
        description: format!("Generated from recording: {}", recording.name),
        steps: steps?,
        variables: HashMap::new(),
        created_at: now,
        updated_at: now,
    };

    Ok(workflow)
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
    // Add a very early log that should always appear
    println!("========================================");
    println!("stop_recording Tauri command called!");
    println!("  profile_id: {}", profile_id);
    println!("  name: {}", name);
    println!("  description: {}", description);
    println!("========================================");
    tracing::info!("========================================");
    tracing::info!("stop_recording called!");
    tracing::info!("  profile_id: {}", profile_id);
    tracing::info!("  name: {}", name);
    tracing::info!("  description: {}", description);
    tracing::info!("========================================");

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
                for (idx, entry) in console_log.iter().enumerate() {
                    tracing::info!("Checking entry #{}: {}", idx, entry);
                    if let Some(args) = entry.get("args").and_then(|a| a.as_array()) {
                        if args.len() >= 2 && args[0] == "__BROWSION_EVENT__" {
                            browsion_event_count += 1;
                            tracing::info!("✓ Found BROWSION event #{}: args[1] = {}", browsion_event_count, args[1]);
                            if args[1].is_string() {
                                let event_str = args[1].as_str().unwrap_or("");
                                if let Ok(event_data) = serde_json::from_str::<serde_json::Value>(event_str) {
                                    tracing::info!("  ✓ Parsed event data: {:?}", event_data);
                                    if let (Some(event_type), Some(data)) = (
                                        event_data.get("type").and_then(|t| t.as_str()),
                                        event_data.get("data")
                                    ) {
                                        let action_type = match event_type {
                                            "click" => Some(crate::recording::RecordedActionType::Click),
                                            "input" => Some(crate::recording::RecordedActionType::Type),
                                            "change" => Some(crate::recording::RecordedActionType::Type),
                                            "keydown" => Some(crate::recording::RecordedActionType::PressKey),
                                            _ => None,
                                        };

                                        if let Some(at) = action_type {
                                            match state.recording_session_manager.add_action(
                                                &profile_id,
                                                at.clone(),
                                                data.clone(),
                                            ) {
                                                Ok(_) => {
                                                    tracing::info!("  ✓ Successfully added action: {:?}", at);
                                                }
                                                Err(e) => {
                                                    tracing::error!("  ✗ Failed to add action: {}", e);
                                                }
                                            }
                                        } else {
                                            tracing::warn!("  ✗ Unknown event type: {}", event_type);
                                        }
                                    } else {
                                        tracing::warn!("  ✗ Event missing type or data: {:?}", event_data);
                                    }
                                } else {
                                    tracing::error!("  ✗ Failed to parse event JSON: {}", event_str);
                                }
                            } else {
                                tracing::warn!("  ✗ Event args[1] is not a string: {:?}", args[1]);
                            }
                        }
                    }
                }
                tracing::info!("Total: found {} BROWSION events in console log", browsion_event_count);

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

    tracing::info!("Calling stop_session...");
    let mut recording = state
        .recording_session_manager
        .stop_session(&profile_id)?;

    tracing::info!("Recording has {} actions", recording.actions.len());

    // Update metadata
    recording.name = name;
    recording.description = description;

    // Save recording
    state
        .recording_manager
        .save(recording.clone())
        .map_err(|e| e.to_string())?;

    // Emit event for UI update
    state.emit("recording-status-changed");

    tracing::info!("Recording saved: {} with {} actions", recording.name, recording.actions.len());
    tracing::info!("========================================");

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
