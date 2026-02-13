use crate::config::AppConfig;
use crate::process::ProcessManager;
use parking_lot::RwLock;
use std::sync::Arc;

/// Application global state
pub struct AppState {
    /// Application configuration
    pub config: Arc<RwLock<AppConfig>>,

    /// Process manager
    pub process_manager: Arc<ProcessManager>,
}

impl AppState {
    pub fn new(config: AppConfig) -> Self {
        Self {
            config: Arc::new(RwLock::new(config)),
            process_manager: Arc::new(ProcessManager::new()),
        }
    }
}
