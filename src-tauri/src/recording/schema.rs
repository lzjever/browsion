//! Recording data structures for browser automation playback.

use serde::{Deserialize, Serialize};

/// A recording of user actions in a browser session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Recording {
    pub id: String,
    pub name: String,
    pub description: String,
    pub profile_id: String,
    pub actions: Vec<RecordedAction>,
    pub created_at: u64,
    pub duration_ms: u64,
}

/// A single recorded action.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordedAction {
    /// Sequential index in the recording.
    pub index: usize,
    /// Type of action.
    #[serde(rename = "type")]
    pub action_type: RecordedActionType,
    /// Action-specific parameters.
    pub params: serde_json::Value,
    /// Timestamp when action occurred (relative to recording start, in ms).
    pub timestamp_ms: u64,
    /// Optional screenshot data after action.
    pub screenshot_base64: Option<String>,
}

/// Types of actions that can be recorded.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RecordedActionType {
    // Navigation
    Navigate,
    GoBack,
    GoForward,
    Reload,

    // Mouse
    Click,
    Hover,
    DoubleClick,
    RightClick,

    // Keyboard
    Type,
    SlowType,
    PressKey,

    // Forms
    SelectOption,
    UploadFile,

    // Scroll
    Scroll,
    ScrollIntoView,

    // Tabs
    NewTab,
    SwitchTab,
    CloseTab,

    // Wait
    Sleep,
    WaitForText,
    WaitForElement,

    // Screenshot
    Screenshot,

    // Console
    GetConsoleLogs,

    // Extract
    Extract,
}

/// Public session info for API responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingSessionInfo {
    pub id: String,
    pub profile_id: String,
    pub started_at: u64,
    pub action_count: usize,
    pub is_recording: bool,
}

/// An active recording session.
#[derive(Debug, Clone)]
pub struct RecordingSession {
    pub id: String,
    pub profile_id: String,
    pub started_at: u64,
    pub actions: Vec<RecordedAction>,
    pub is_recording: bool,
}

impl RecordingSession {
    pub fn new(profile_id: String) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            profile_id,
            started_at: now_ms(),
            actions: Vec::new(),
            is_recording: true,
        }
    }

    pub fn add_action(&mut self, action_type: RecordedActionType, params: serde_json::Value) {
        let action = RecordedAction {
            index: self.actions.len(),
            action_type,
            params,
            timestamp_ms: now_ms().saturating_sub(self.started_at),
            screenshot_base64: None,
        };
        self.actions.push(action);
    }

    pub fn finish(self) -> Recording {
        let duration_ms = now_ms().saturating_sub(self.started_at);
        Recording {
            id: self.id.clone(),
            name: format!("Recording {}", &self.id[..8]),
            description: String::new(),
            profile_id: self.profile_id,
            actions: self.actions,
            created_at: self.started_at,
            duration_ms,
        }
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_recording_serialization() {
        let recording = Recording {
            id: "test-recording".to_string(),
            name: "Test Recording".to_string(),
            description: "A test recording".to_string(),
            profile_id: "profile-1".to_string(),
            actions: vec![],
            created_at: 0,
            duration_ms: 1000,
        };

        let json = serde_json::to_string(&recording).unwrap();
        let _parsed: Recording = serde_json::from_str(&json).unwrap();
    }

    #[test]
    fn test_recording_session() {
        let mut session = RecordingSession::new("profile-1".to_string());

        session.add_action(
            RecordedActionType::Navigate,
            serde_json::json!({ "url": "https://example.com" }),
        );

        session.add_action(
            RecordedActionType::Click,
            serde_json::json!({ "selector": "#submit" }),
        );

        assert_eq!(session.actions.len(), 2);
        assert_eq!(session.actions[0].action_type, RecordedActionType::Navigate);
        assert_eq!(session.actions[1].action_type, RecordedActionType::Click);
    }

    #[test]
    fn test_recording_with_no_actions() {
        let r = Recording {
            id: "empty-rec".to_string(),
            name: "Empty".to_string(),
            description: "".to_string(),
            profile_id: "p1".to_string(),
            actions: vec![],
            created_at: 0,
            duration_ms: 0,
        };
        let json = serde_json::to_string(&r).unwrap();
        let parsed: Recording = serde_json::from_str(&json).unwrap();
        assert!(parsed.actions.is_empty());
        assert_eq!(parsed.duration_ms, 0);
    }

    #[test]
    fn test_all_recorded_action_types_serialize() {
        use crate::recording::RecordedActionType;
        let types = [
            RecordedActionType::Navigate,
            RecordedActionType::Click,
            RecordedActionType::Type,
            RecordedActionType::Screenshot,
            RecordedActionType::NewTab,
            RecordedActionType::Scroll,
        ];
        for t in &types {
            let json = serde_json::to_string(t).unwrap();
            let parsed: RecordedActionType = serde_json::from_str(&json).unwrap();
            assert_eq!(&parsed, t);
        }
    }
}
