use crate::agent::SessionManager;
use crate::api::action_log::ActionLog;
use crate::api::ws::WsBroadcaster;
use crate::config::AppConfig;
use crate::process::ProcessManager;
use crate::recording::{RecordingManager, RecordingSessionManager};
use crate::workflow::WorkflowManager;
use parking_lot::{Mutex, RwLock};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};

/// Application global state
#[derive(Clone)]
pub struct AppState {
    pub config: Arc<RwLock<AppConfig>>,
    pub process_manager: Arc<ProcessManager>,
    pub session_manager: Arc<SessionManager>,
    /// Abort callback for the running API server task, for runtime stop/restart.
    pub api_server_abort: Arc<std::sync::Mutex<Option<Box<dyn FnOnce() + Send>>>>,
    /// Tauri app handle for emitting events to the frontend.
    pub app_handle: Arc<Mutex<Option<AppHandle>>>,
    /// In-memory action log (last 2000 API calls).
    pub action_log: Arc<ActionLog>,
    /// WebSocket broadcaster for real-time events.
    pub ws_broadcaster: WsBroadcaster,
    /// Workflow manager for automation workflows.
    pub workflow_manager: Arc<WorkflowManager>,
    /// Recording manager for browser action recordings.
    pub recording_manager: Arc<RecordingManager>,
    /// Active recording session manager.
    pub recording_session_manager: Arc<RecordingSessionManager>,
}

impl AppState {
    pub fn new(config: AppConfig) -> Self {
        let recent = config.recent_profiles.clone();
        // Create recording_session_manager first, then use it for session_manager
        let recording_session_manager = Arc::new(RecordingSessionManager::new());
        let session_manager = Arc::new(SessionManager::with_recording(
            Arc::clone(&recording_session_manager)
        ));
        Self {
            config: Arc::new(RwLock::new(config)),
            process_manager: Arc::new(ProcessManager::new_with_recent(recent)),
            session_manager,
            api_server_abort: Arc::new(std::sync::Mutex::new(None)),
            app_handle: Arc::new(Mutex::new(None)),
            action_log: Arc::new(ActionLog::new()),
            ws_broadcaster: WsBroadcaster::new(),
            workflow_manager: Arc::new(WorkflowManager::new().unwrap_or_default()),
            recording_manager: Arc::new(RecordingManager::new().unwrap_or_default()),
            recording_session_manager,
        }
    }

    /// Emit a Tauri event to all frontend windows. No-op if app handle is not yet set.
    pub fn emit(&self, event: &str) {
        if let Some(handle) = &*self.app_handle.lock() {
            let _ = handle.emit(event, ());
        }
    }

    /// Broadcast a WebSocket event to all connected clients.
    pub fn broadcast_ws(&self, event: crate::api::ws::WsEvent) {
        self.ws_broadcaster.broadcast(event);
    }
}
