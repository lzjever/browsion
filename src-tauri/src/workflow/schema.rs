//! Workflow data structures and serialization.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A workflow definition: a reusable sequence of automation steps.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workflow {
    pub id: String,
    pub name: String,
    pub description: String,
    pub steps: Vec<WorkflowStep>,
    /// Default variables that can be overridden at execution time.
    #[serde(default)]
    pub variables: HashMap<String, serde_json::Value>,
    /// Created at timestamp (Unix ms).
    pub created_at: u64,
    /// Updated at timestamp (Unix ms).
    pub updated_at: u64,
}

/// A single step in a workflow.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowStep {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    /// Step type determines which handler executes it.
    #[serde(rename = "type")]
    pub step_type: StepType,
    /// Step-specific parameters (will be validated against step type).
    pub params: serde_json::Value,
    /// Continue to next step even if this step fails.
    #[serde(default)]
    pub continue_on_error: bool,
    /// Step timeout in milliseconds (default 30000).
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
}

fn default_timeout() -> u64 {
    30000
}

/// Supported step types.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StepType {
    // Navigation
    Navigate,
    GoBack,
    GoForward,
    Reload,
    WaitForUrl,
    WaitForNavigation,

    // Mouse
    Click,
    ClickAt,
    Hover,
    DoubleClick,
    RightClick,
    Drag,

    // Keyboard
    Type,
    SlowType,
    PressKey,

    // Forms
    SelectOption,
    UploadFile,

    // Scroll & Wait
    Scroll,
    ScrollElement,
    ScrollIntoView,
    WaitForElement,
    WaitForText,

    // Observation
    Screenshot,
    ScreenshotElement,
    GetPageState,
    GetPageText,
    GetCookies,
    Extract,

    // Tabs
    NewTab,
    SwitchTab,
    CloseTab,
    WaitForNewTab,

    // Console
    GetConsoleLogs,

    // Control
    Sleep,
    SetVariable,
    // Condition: if variable matches value, run sub-steps
    Condition,
}

impl std::fmt::Display for StepType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            StepType::Navigate => "navigate",
            StepType::GoBack => "go_back",
            StepType::GoForward => "go_forward",
            StepType::Reload => "reload",
            StepType::WaitForUrl => "wait_for_url",
            StepType::WaitForNavigation => "wait_for_navigation",
            StepType::Click => "click",
            StepType::ClickAt => "click_at",
            StepType::Hover => "hover",
            StepType::DoubleClick => "double_click",
            StepType::RightClick => "right_click",
            StepType::Drag => "drag",
            StepType::Type => "type",
            StepType::SlowType => "slow_type",
            StepType::PressKey => "press_key",
            StepType::SelectOption => "select_option",
            StepType::UploadFile => "upload_file",
            StepType::Scroll => "scroll",
            StepType::ScrollElement => "scroll_element",
            StepType::ScrollIntoView => "scroll_into_view",
            StepType::WaitForElement => "wait_for_element",
            StepType::WaitForText => "wait_for_text",
            StepType::Screenshot => "screenshot",
            StepType::ScreenshotElement => "screenshot_element",
            StepType::GetPageState => "get_page_state",
            StepType::GetPageText => "get_page_text",
            StepType::GetCookies => "get_cookies",
            StepType::Extract => "extract",
            StepType::NewTab => "new_tab",
            StepType::SwitchTab => "switch_tab",
            StepType::CloseTab => "close_tab",
            StepType::WaitForNewTab => "wait_for_new_tab",
            StepType::GetConsoleLogs => "get_console_logs",
            StepType::Sleep => "sleep",
            StepType::SetVariable => "set_variable",
            StepType::Condition => "condition",
        };
        write!(f, "{}", s)
    }
}

impl From<crate::recording::RecordedActionType> for StepType {
    fn from(value: crate::recording::RecordedActionType) -> Self {
        match value {
            crate::recording::RecordedActionType::Navigate => StepType::Navigate,
            crate::recording::RecordedActionType::GoBack => StepType::GoBack,
            crate::recording::RecordedActionType::GoForward => StepType::GoForward,
            crate::recording::RecordedActionType::Reload => StepType::Reload,
            crate::recording::RecordedActionType::Click => StepType::Click,
            crate::recording::RecordedActionType::Hover => StepType::Hover,
            crate::recording::RecordedActionType::DoubleClick => StepType::DoubleClick,
            crate::recording::RecordedActionType::RightClick => StepType::RightClick,
            crate::recording::RecordedActionType::Type => StepType::Type,
            crate::recording::RecordedActionType::SlowType => StepType::SlowType,
            crate::recording::RecordedActionType::PressKey => StepType::PressKey,
            crate::recording::RecordedActionType::SelectOption => StepType::SelectOption,
            crate::recording::RecordedActionType::UploadFile => StepType::UploadFile,
            crate::recording::RecordedActionType::Scroll => StepType::Scroll,
            crate::recording::RecordedActionType::ScrollIntoView => StepType::ScrollIntoView,
            crate::recording::RecordedActionType::NewTab => StepType::NewTab,
            crate::recording::RecordedActionType::SwitchTab => StepType::SwitchTab,
            crate::recording::RecordedActionType::CloseTab => StepType::CloseTab,
            crate::recording::RecordedActionType::Sleep => StepType::Sleep,
            crate::recording::RecordedActionType::WaitForText => StepType::WaitForText,
            crate::recording::RecordedActionType::WaitForElement => StepType::WaitForElement,
            crate::recording::RecordedActionType::Screenshot => StepType::Screenshot,
            crate::recording::RecordedActionType::GetConsoleLogs => StepType::GetConsoleLogs,
            crate::recording::RecordedActionType::Extract => StepType::Extract,
        }
    }
}

/// A workflow execution instance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowExecution {
    pub id: String,
    pub workflow_id: String,
    pub profile_id: String,
    pub status: ExecutionStatus,
    pub current_step_index: usize,
    pub step_results: Vec<StepResult>,
    /// Runtime variables (merged from workflow defaults + execution overrides).
    pub variables: HashMap<String, serde_json::Value>,
    pub started_at: u64,
    pub completed_at: Option<u64>,
    pub error: Option<String>,
}

impl WorkflowExecution {
    /// Create a new execution instance.
    pub fn new(
        workflow_id: String,
        profile_id: String,
        variables: HashMap<String, serde_json::Value>,
        total_steps: usize,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            workflow_id,
            profile_id,
            status: ExecutionStatus::Pending,
            current_step_index: 0,
            step_results: Vec::with_capacity(total_steps),
            variables,
            started_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            completed_at: None,
            error: None,
        }
    }
}

/// Execution status.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Paused,
    Cancelled,
}

/// Result of a single step execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepResult {
    pub step_id: String,
    pub status: ExecutionStatus,
    pub duration_ms: u64,
    pub output: Option<serde_json::Value>,
    pub error: Option<String>,
    pub started_at: u64,
    pub completed_at: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_workflow_serialization() {
        let workflow = Workflow {
            id: "test-workflow".to_string(),
            name: "Test Workflow".to_string(),
            description: "A test workflow".to_string(),
            steps: vec![WorkflowStep {
                id: "step1".to_string(),
                name: "Navigate".to_string(),
                description: String::new(),
                step_type: StepType::Navigate,
                params: serde_json::json!({ "url": "https://example.com" }),
                continue_on_error: false,
                timeout_ms: 30000,
            }],
            variables: HashMap::new(),
            created_at: 0,
            updated_at: 0,
        };

        let json = serde_json::to_string(&workflow).unwrap();
        let _parsed: Workflow = serde_json::from_str(&json).unwrap();
    }

    #[test]
    fn test_workflow_empty_steps_valid() {
        let w = Workflow {
            id: "empty".to_string(),
            name: "Empty Workflow".to_string(),
            description: "".to_string(),
            steps: vec![],
            variables: std::collections::HashMap::new(),
            created_at: 0,
            updated_at: 0,
        };
        let json = serde_json::to_string(&w).unwrap();
        let parsed: Workflow = serde_json::from_str(&json).unwrap();
        assert!(parsed.steps.is_empty());
    }

    #[test]
    fn test_workflow_variables_roundtrip() {
        let mut vars = std::collections::HashMap::new();
        vars.insert("url".to_string(), serde_json::Value::String("https://example.com".to_string()));
        vars.insert("timeout".to_string(), serde_json::Value::Number(5000.into()));
        let w = Workflow {
            id: "var-test".to_string(),
            name: "Var Test".to_string(),
            description: "".to_string(),
            steps: vec![],
            variables: vars,
            created_at: 100,
            updated_at: 200,
        };
        let json = serde_json::to_string(&w).unwrap();
        let parsed: Workflow = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.variables.get("url").unwrap(), "https://example.com");
        assert_eq!(parsed.variables.get("timeout").unwrap(), &serde_json::Value::Number(5000.into()));
    }

    #[test]
    fn test_step_type_serialization_roundtrip() {
        for st in [StepType::Navigate, StepType::Click, StepType::Type, StepType::Screenshot] {
            let json = serde_json::to_string(&st).unwrap();
            let parsed: StepType = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed, st);
        }
    }
}
