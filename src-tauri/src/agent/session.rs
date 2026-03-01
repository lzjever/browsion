//! Persistent CDP connection pool keyed by profile_id.
//! Provides lazy connect, auto-reconnect, and cleanup.

use crate::agent::cdp::CDPClient;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

type ClientHandle = Arc<Mutex<CDPClient>>;

pub struct SessionManager {
    sessions: Mutex<HashMap<String, ClientHandle>>,
    /// Recording session manager for browser-level event recording (optional)
    recording_session_manager: Option<Arc<crate::recording::session::RecordingSessionManager>>,
}

impl SessionManager {
    pub fn new() -> Self {
        Self {
            sessions: Mutex::new(HashMap::new()),
            recording_session_manager: None,
        }
    }

    /// Create a new SessionManager with recording session manager support.
    pub fn with_recording(
        recording_session_manager: Arc<crate::recording::session::RecordingSessionManager>,
    ) -> Self {
        Self {
            sessions: Mutex::new(HashMap::new()),
            recording_session_manager: Some(recording_session_manager),
        }
    }

    /// Set the recording session manager (for late binding).
    pub fn set_recording_manager(&mut self, manager: Arc<crate::recording::session::RecordingSessionManager>) {
        self.recording_session_manager = Some(manager);
    }

    /// Get (or create) a per-profile CDP client handle.
    /// The returned `Arc<Mutex<CDPClient>>` can be locked independently of
    /// the session map, so multiple profiles can operate in parallel.
    pub async fn get_client(
        &self,
        profile_id: &str,
        cdp_port: u16,
    ) -> Result<ClientHandle, String> {
        {
            let sessions = self.sessions.lock().await;
            if let Some(handle) = sessions.get(profile_id) {
                if handle.lock().await.is_connected() {
                    return Ok(Arc::clone(handle));
                }
            }
        }

        // Need to (re)connect
        let client = if let Some(ref manager) = self.recording_session_manager {
            CDPClient::attach_with_recording(profile_id.to_string(), cdp_port, Some(Arc::clone(manager))).await?
        } else {
            CDPClient::attach(profile_id.to_string(), cdp_port).await?
        };
        let handle: ClientHandle = Arc::new(Mutex::new(client));
        let mut sessions = self.sessions.lock().await;
        sessions.insert(profile_id.to_string(), Arc::clone(&handle));
        Ok(handle)
    }

    /// Disconnect a single profile's CDP session.
    pub async fn disconnect(&self, profile_id: &str) {
        let mut sessions = self.sessions.lock().await;
        if let Some(handle) = sessions.remove(profile_id) {
            let mut client = handle.lock().await;
            let _ = client.close().await;
            tracing::info!("Disconnected CDP session for {}", profile_id);
        }
    }

    /// Disconnect all sessions (e.g. on app shutdown).
    pub async fn disconnect_all(&self) {
        let mut sessions = self.sessions.lock().await;
        for (id, handle) in sessions.drain() {
            let mut client = handle.lock().await;
            let _ = client.close().await;
            tracing::info!("Disconnected CDP session for {}", id);
        }
    }

    /// Check if a profile has an active CDP connection.
    pub async fn is_connected(&self, profile_id: &str) -> bool {
        let sessions = self.sessions.lock().await;
        if let Some(handle) = sessions.get(profile_id) {
            handle.lock().await.is_connected()
        } else {
            false
        }
    }
}

impl Default for SessionManager {
    fn default() -> Self {
        Self::new()
    }
}
