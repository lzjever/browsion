use crate::api::ws::WsBroadcaster;
use crate::config::AppConfig;
use crate::process::ProcessManager;
use parking_lot::{Mutex, RwLock};
use tauri::{AppHandle, Emitter};

/// Shared application state.
pub struct AppState {
    pub config: RwLock<AppConfig>,
    pub process_manager: ProcessManager,
    pub app_handle: Mutex<Option<AppHandle>>,
    pub api_server_abort: Mutex<Option<Box<dyn Fn() + Send + Sync>>>,
    pub ws_broadcaster: WsBroadcaster,
}

impl AppState {
    pub fn new(config: AppConfig) -> Self {
        Self {
            config: RwLock::new(config),
            process_manager: ProcessManager::new(),
            app_handle: Mutex::new(None),
            api_server_abort: Mutex::new(None),
            ws_broadcaster: WsBroadcaster::new(),
        }
    }

    /// Emit an event to the Tauri frontend.
    pub fn emit(&self, event: &str) {
        if let Some(handle) = self.app_handle.lock().as_ref() {
            if let Err(e) = handle.emit(event, ()) {
                tracing::warn!("Failed to emit event {}: {}", event, e);
            }
        }
    }
}
