use crate::agent::engine::AgentEngine;
use crate::agent::types::AIConfig as AgentAIConfig;
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

    /// Agent engine for AI automation
    pub agent_engine: Arc<AgentEngine>,
}

impl AppState {
    pub fn new(config: AppConfig) -> Self {
        // Convert config AIConfig to agent AIConfig
        let ai_config = AgentAIConfig::from(config.ai.clone());

        Self {
            config: Arc::new(RwLock::new(config)),
            process_manager: Arc::new(ProcessManager::new()),
            agent_engine: Arc::new(AgentEngine::new(ai_config)),
        }
    }
}
