//! Tauri commands for workflow management.

use crate::state::AppState;
use crate::workflow::{StepType, Workflow, WorkflowExecution};
use std::collections::HashMap;
use std::sync::Arc;

#[tauri::command]
pub async fn list_workflows(state: tauri::State<'_, Arc<AppState>>) -> Result<Vec<Workflow>, String> {
    Ok(state.workflow_manager.list())
}

#[tauri::command]
pub async fn get_workflow(
    id: String,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<Workflow, String> {
    state
        .workflow_manager
        .get(&id)
        .ok_or_else(|| "Workflow not found".to_string())
}

#[tauri::command]
pub async fn save_workflow(
    workflow: Workflow,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<Workflow, String> {
    state
        .workflow_manager
        .save(workflow.clone())
        .map_err(|e| e.to_string())?;
    Ok(workflow)
}

#[tauri::command]
pub async fn delete_workflow(
    id: String,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<(), String> {
    state
        .workflow_manager
        .delete(&id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn run_workflow(
    workflow_id: String,
    profile_id: String,
    variables: HashMap<String, serde_json::Value>,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<WorkflowExecution, String> {
    // Get workflow
    let workflow = state
        .workflow_manager
        .get(&workflow_id)
        .ok_or_else(|| "Workflow not found".to_string())?;

    // Get API config
    let config = state.config.read().clone();
    let mcp = config.mcp.clone();
    if !mcp.enabled {
        return Err("API server not enabled".to_string());
    }

    // Launch browser (or attach to existing session) before running workflow
    let api_base = format!("http://127.0.0.1:{}", mcp.api_port);
    let launch_url = format!("{}/api/launch/{}", api_base, profile_id);

    // Check if browser is already running
    let is_running = state.process_manager.is_running(&profile_id);

    if !is_running {
        // Launch browser
        let client = reqwest::Client::new();
        let mut req = client.post(&launch_url);
        if let Some(ref key) = mcp.api_key {
            req = req.header("X-API-Key", key);
        }

        let resp = req.send().await.map_err(|e| format!("Failed to launch browser: {}", e))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let error_text = resp.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(format!("Failed to launch browser: HTTP {} - {}", status, error_text));
        }
    }

    // Create executor
    let executor = crate::workflow::WorkflowExecutor::new(
        state.session_manager.clone(),
        mcp.api_port,
        mcp.api_key.clone(),
    );

    // Execute workflow
    let execution = executor
        .execute(&workflow, profile_id, variables)
        .await;

    Ok(execution)
}

#[tauri::command]
pub async fn validate_workflow_step(step: serde_json::Value) -> Result<bool, String> {
    // Basic validation: check if step has required fields
    let step_type = step
        .get("type")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Missing step type".to_string())?;

    let step_type: StepType = serde_json::from_value(serde_json::json!(step_type))
        .map_err(|e| format!("Invalid step type: {}", e))?;

    // Validate params based on step type
    match step_type {
        StepType::Navigate => {
            if step
                .get("url")
                .and_then(|v| v.as_str())
                .is_none_or(|s| s.is_empty())
            {
                return Err("Navigate step requires 'url' parameter".to_string());
            }
        }
        StepType::Click | StepType::Hover => {
            if step
                .get("selector")
                .and_then(|v| v.as_str())
                .is_none_or(|s| s.is_empty())
            {
                return Err(format!("{} step requires 'selector' parameter", step_type));
            }
        }
        StepType::Type => {
            if step
                .get("selector")
                .and_then(|v| v.as_str())
                .is_none_or(|s| s.is_empty())
            {
                return Err(format!("{} step requires 'selector' parameter", step_type));
            }
            if step
                .get("text")
                .and_then(|v| v.as_str())
                .is_none_or(|s| s.is_empty())
            {
                return Err("Type step requires 'text' parameter".to_string());
            }
        }
        _ => {
            // Other step types have optional or no params
        }
    }

    Ok(true)
}

/// Get list of available step types with descriptions.
#[tauri::command]
pub async fn get_step_types() -> Vec<StepTypeInfo> {
    vec![
        StepTypeInfo {
            type_: StepType::Navigate,
            name: "Navigate".to_string(),
            description: "Navigate to a URL".to_string(),
            params: vec![ParamInfo {
                name: "url".to_string(),
                type_: "string".to_string(),
                required: true,
                description: "URL to navigate to".to_string(),
            }],
        },
        StepTypeInfo {
            type_: StepType::Click,
            name: "Click".to_string(),
            description: "Click an element".to_string(),
            params: vec![ParamInfo {
                name: "selector".to_string(),
                type_: "string".to_string(),
                required: true,
                description: "CSS selector for the element".to_string(),
            }],
        },
        StepTypeInfo {
            type_: StepType::Type,
            name: "Type".to_string(),
            description: "Type text into an input".to_string(),
            params: vec![
                ParamInfo {
                    name: "selector".to_string(),
                    type_: "string".to_string(),
                    required: true,
                    description: "CSS selector for the input".to_string(),
                },
                ParamInfo {
                    name: "text".to_string(),
                    type_: "string".to_string(),
                    required: true,
                    description: "Text to type".to_string(),
                },
            ],
        },
        StepTypeInfo {
            type_: StepType::Sleep,
            name: "Sleep".to_string(),
            description: "Wait for a duration".to_string(),
            params: vec![ParamInfo {
                name: "duration_ms".to_string(),
                type_: "number".to_string(),
                required: false,
                description: "Duration in milliseconds (default 1000)".to_string(),
            }],
        },
        StepTypeInfo {
            type_: StepType::Screenshot,
            name: "Screenshot".to_string(),
            description: "Take a screenshot".to_string(),
            params: vec![],
        },
        StepTypeInfo {
            type_: StepType::GetPageState,
            name: "Get Page State".to_string(),
            description: "Get current page URL, title, and AX tree".to_string(),
            params: vec![],
        },
        StepTypeInfo {
            type_: StepType::WaitForText,
            name: "Wait For Text".to_string(),
            description: "Wait for text to appear on page".to_string(),
            params: vec![
                ParamInfo {
                    name: "text".to_string(),
                    type_: "string".to_string(),
                    required: true,
                    description: "Text to wait for".to_string(),
                },
                ParamInfo {
                    name: "timeout_ms".to_string(),
                    type_: "number".to_string(),
                    required: false,
                    description: "Timeout in milliseconds (default 10000)".to_string(),
                },
            ],
        },
    ]
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StepTypeInfo {
    #[serde(rename = "type")]
    pub type_: StepType,
    pub name: String,
    pub description: String,
    pub params: Vec<ParamInfo>,
}

#[derive(serde::Serialize)]
pub struct ParamInfo {
    pub name: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub required: bool,
    pub description: String,
}
