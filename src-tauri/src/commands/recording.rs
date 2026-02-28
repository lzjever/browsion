//! Tauri commands for recording management.

use crate::recording::{Recording, RecordingManager};
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
