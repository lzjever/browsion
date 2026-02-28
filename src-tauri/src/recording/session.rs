//! Active recording session management.

use crate::recording::schema::{Recording, RecordingSession, RecordedActionType, RecordingSessionInfo};
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::Arc;

/// Manages active recording sessions.
pub struct RecordingSessionManager {
    sessions: Arc<Mutex<HashMap<String, RecordingSession>>>,
}

impl RecordingSessionManager {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Start a new recording session for a profile.
    pub fn start_session(&self, profile_id: String) -> Result<String, String> {
        let mut sessions = self.sessions.lock();

        if sessions.contains_key(&profile_id) {
            return Err(format!("Recording already in progress for profile {}", profile_id));
        }

        let session = RecordingSession::new(profile_id.clone());
        let session_id = session.id.clone();
        sessions.insert(profile_id, session);

        Ok(session_id)
    }

    /// Stop an active recording session and return the recording.
    pub fn stop_session(&self, profile_id: &str) -> Result<Recording, String> {
        let mut sessions = self.sessions.lock();

        let session = sessions
            .remove(profile_id)
            .ok_or_else(|| format!("No active recording for profile {}", profile_id))?;

        if !session.is_recording {
            return Err("Session was already stopped".to_string());
        }

        Ok(session.finish())
    }

    /// Add an action to an active recording session.
    pub fn add_action(
        &self,
        profile_id: &str,
        action_type: RecordedActionType,
        params: serde_json::Value,
    ) -> Result<(), String> {
        let mut sessions = self.sessions.lock();

        let session = sessions
            .get_mut(profile_id)
            .ok_or_else(|| format!("No active recording for profile {}", profile_id))?;

        session.add_action(action_type, params);

        Ok(())
    }

    /// Check if a profile has an active recording session.
    pub fn is_recording(&self, profile_id: &str) -> bool {
        let sessions = self.sessions.lock();
        sessions.get(profile_id).is_some_and(|s| s.is_recording)
    }

    /// Get all active recording sessions.
    pub fn get_active_sessions(&self) -> HashMap<String, String> {
        let sessions = self.sessions.lock();
        sessions
            .iter()
            .filter(|(_, s)| s.is_recording)
            .map(|(profile_id, session)| (profile_id.clone(), session.id.clone()))
            .collect()
    }

    /// Get session info for a specific profile.
    pub fn get_session_info(&self, profile_id: &str) -> Option<RecordingSessionInfo> {
        let sessions = self.sessions.lock();
        sessions.get(profile_id).map(|session| RecordingSessionInfo {
            id: session.id.clone(),
            profile_id: session.profile_id.clone(),
            started_at: session.started_at,
            action_count: session.actions.len(),
            is_recording: session.is_recording,
        })
    }
}

impl Default for RecordingSessionManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_lifecycle() {
        let manager = RecordingSessionManager::new();

        // Start session
        let session_id = manager.start_session("profile-1".to_string()).unwrap();
        assert!(manager.is_recording("profile-1"));

        // Add action
        manager
            .add_action(
                "profile-1",
                RecordedActionType::Navigate,
                serde_json::json!({ "url": "https://example.com" }),
            )
            .unwrap();

        let info = manager.get_session_info("profile-1").unwrap();
        assert_eq!(info.action_count, 1);

        // Stop session
        let recording = manager.stop_session("profile-1").unwrap();
        assert!(!manager.is_recording("profile-1"));
        assert_eq!(recording.actions.len(), 1);
    }

    #[test]
    fn test_double_start_fails() {
        let manager = RecordingSessionManager::new();

        manager.start_session("profile-1".to_string()).unwrap();

        let result = manager.start_session("profile-1".to_string());
        assert!(result.is_err());
    }
}
