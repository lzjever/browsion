use crate::agent::llm::test_provider;
use crate::agent::types::{AgentOptions, AgentProgress};
use crate::config::{validation, AIConfig, BrowserProfile, ProviderConfig};
use crate::state::AppState;
use crate::window;
use std::collections::HashMap;
use std::path::PathBuf;
use tauri::State;

/// Get all profiles
#[tauri::command]
pub async fn get_profiles(state: State<'_, AppState>) -> Result<Vec<BrowserProfile>, String> {
    let config = state.config.read();
    Ok(config.profiles.clone())
}

/// Get the current Chrome path
#[tauri::command]
pub async fn get_chrome_path(state: State<'_, AppState>) -> Result<String, String> {
    let config = state.config.read();
    Ok(config.chrome_path.display().to_string())
}

/// Launch a profile
#[tauri::command]
pub async fn launch_profile(profile_id: String, state: State<'_, AppState>) -> Result<u32, String> {
    let config = state.config.read().clone();
    let pid = state
        .process_manager
        .launch_profile(&profile_id, &config)
        .await
        .map_err(|e| e.to_string())?;

    // Update recent profiles in config
    {
        let mut config = state.config.write();
        // Remove if already exists
        config.recent_profiles.retain(|id| id != &profile_id);
        // Add to front
        config.recent_profiles.insert(0, profile_id.clone());
        // Keep only last 10
        if config.recent_profiles.len() > 10 {
            config.recent_profiles.truncate(10);
        }
        // Save to disk
        if let Err(e) = crate::config::save_config(&config) {
            tracing::warn!("Failed to save recent profiles: {}", e);
        }
    }

    Ok(pid)
}

/// Activate (focus) a running profile's window
#[tauri::command]
pub async fn activate_profile(
    profile_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    if let Some(info) = state.process_manager.get_process_info(&profile_id) {
        window::activate_window(info.pid).map_err(|e| e.to_string())?;
        Ok(())
    } else {
        Err(format!("Profile {} is not running", profile_id))
    }
}

/// Kill a running profile
#[tauri::command]
pub async fn kill_profile(profile_id: String, state: State<'_, AppState>) -> Result<(), String> {
    state
        .process_manager
        .kill_profile(&profile_id)
        .await
        .map_err(|e| e.to_string())
}

/// Get running status for all profiles
#[tauri::command]
pub async fn get_running_profiles(
    state: State<'_, AppState>,
) -> Result<HashMap<String, bool>, String> {
    let config = state.config.read();
    let mut status = HashMap::new();

    for profile in &config.profiles {
        status.insert(
            profile.id.clone(),
            state.process_manager.is_running(&profile.id),
        );
    }

    Ok(status)
}

/// Add a new profile
#[tauri::command]
pub async fn add_profile(
    profile: BrowserProfile,
    state: State<'_, AppState>,
) -> Result<(), String> {
    // Validate profile
    validation::validate_profile(&profile).map_err(|e| e.to_string())?;

    let mut config = state.config.write();
    config.profiles.push(profile);

    // Save to disk
    crate::config::save_config(&config).map_err(|e| e.to_string())?;

    Ok(())
}

/// Update an existing profile
#[tauri::command]
pub async fn update_profile(
    profile: BrowserProfile,
    state: State<'_, AppState>,
) -> Result<(), String> {
    // Validate profile
    validation::validate_profile(&profile).map_err(|e| e.to_string())?;

    let mut config = state.config.write();

    if let Some(p) = config.profiles.iter_mut().find(|p| p.id == profile.id) {
        *p = profile;
        // Save to disk
        crate::config::save_config(&config).map_err(|e| e.to_string())?;
        Ok(())
    } else {
        Err(format!("Profile {} not found", profile.id))
    }
}

/// Delete a profile
#[tauri::command]
pub async fn delete_profile(profile_id: String, state: State<'_, AppState>) -> Result<(), String> {
    // Check if profile is running
    if state.process_manager.is_running(&profile_id) {
        return Err(format!(
            "Cannot delete profile {}: it is currently running",
            profile_id
        ));
    }

    let mut config = state.config.write();
    let before_len = config.profiles.len();
    config.profiles.retain(|p| p.id != profile_id);

    if config.profiles.len() == before_len {
        return Err(format!("Profile {} not found", profile_id));
    }

    // Save to disk
    crate::config::save_config(&config).map_err(|e| e.to_string())?;

    Ok(())
}

/// Update Chrome executable path
#[tauri::command]
pub async fn update_chrome_path(path: String, state: State<'_, AppState>) -> Result<(), String> {
    let path_buf = PathBuf::from(&path);

    // Validate the Chrome path
    validation::validate_chrome_path(&path_buf).map_err(|e| e.to_string())?;

    let mut config = state.config.write();
    config.chrome_path = path_buf;

    // Save to disk
    crate::config::save_config(&config).map_err(|e| e.to_string())?;

    Ok(())
}

/// Get application settings
#[tauri::command]
pub async fn get_settings(
    state: State<'_, AppState>,
) -> Result<crate::config::AppSettings, String> {
    let config = state.config.read();
    Ok(config.settings.clone())
}

/// Update application settings
#[tauri::command]
pub async fn update_settings(
    settings: crate::config::AppSettings,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut config = state.config.write();
    config.settings = settings;

    // Save to disk
    crate::config::save_config(&config).map_err(|e| e.to_string())?;

    Ok(())
}

/// Get recently launched profiles
#[tauri::command]
pub async fn get_recent_profiles(
    state: State<'_, AppState>,
) -> Result<Vec<BrowserProfile>, String> {
    let recent_ids = state.process_manager.get_recent_launches();
    let config = state.config.read();

    let mut recent_profiles = Vec::new();
    for profile_id in recent_ids {
        if let Some(profile) = config.profiles.iter().find(|p| p.id == profile_id) {
            recent_profiles.push(profile.clone());
        }
    }

    Ok(recent_profiles)
}

// ============================================
// AI Agent Commands
// ============================================

/// Run an AI agent task
#[tauri::command]
pub async fn run_agent(
    profile_id: String,
    task: String,
    options: Option<AgentOptions>,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let config = state.config.read().clone();

    // Find the profile
    let profile = config
        .profiles
        .iter()
        .find(|p| p.id == profile_id)
        .ok_or_else(|| format!("Profile {} not found", profile_id))?
        .clone();

    // Check if AI is configured
    if config.ai.providers.is_empty() {
        return Err("No AI providers configured. Please configure at least one provider in Settings > AI Configuration.".to_string());
    }

    // Check if default LLM is set and the provider has an API key (for non-local providers)
    if let Some(ref default_llm) = config.ai.default_llm {
        // Format is "provider_id:model_name"
        let provider_id = default_llm.split(':').next().unwrap_or(default_llm);
        if let Some(provider) = config.ai.providers.get(provider_id) {
            if provider.api_type != crate::config::ApiType::Ollama && provider.api_key.is_none() {
                return Err(format!(
                    "API key not configured for provider '{}'. Please add your API key in Settings > AI Configuration.",
                    provider.name
                ));
            }
        } else {
            return Err(format!(
                "Default provider '{}' not found. Please configure it in Settings > AI Configuration.",
                provider_id
            ));
        }
    } else {
        return Err("No default LLM configured. Please select a default LLM in Settings > AI Configuration.".to_string());
    }

    let agent_options = options.unwrap_or_default();
    let ai_config = crate::agent::types::AIConfig::from(config.ai.clone());

    state
        .agent_engine
        .run(
            &profile,
            &config.chrome_path,
            task,
            agent_options,
            ai_config,
        )
        .await
        .map_err(|e| e.to_string())
}

/// Stop a running agent
#[tauri::command]
pub async fn stop_agent(agent_id: String, state: State<'_, AppState>) -> Result<(), String> {
    state
        .agent_engine
        .stop(&agent_id)
        .await
        .map_err(|e| e.to_string())
}

/// Pause a running agent
#[tauri::command]
pub async fn pause_agent(agent_id: String, state: State<'_, AppState>) -> Result<(), String> {
    state
        .agent_engine
        .pause(&agent_id)
        .await
        .map_err(|e| e.to_string())
}

/// Resume a paused agent
#[tauri::command]
pub async fn resume_agent(agent_id: String, state: State<'_, AppState>) -> Result<(), String> {
    state
        .agent_engine
        .resume(&agent_id)
        .await
        .map_err(|e| e.to_string())
}

/// Get agent status
#[tauri::command]
pub async fn get_agent_status(
    agent_id: String,
    state: State<'_, AppState>,
) -> Result<Option<AgentProgress>, String> {
    Ok(state.agent_engine.get_status(&agent_id).await)
}

// ============================================
// AI Configuration Commands
// ============================================

/// Get AI configuration
#[tauri::command]
pub async fn get_ai_config(state: State<'_, AppState>) -> Result<AIConfig, String> {
    let config = state.config.read();
    Ok(config.ai.clone())
}

/// Update AI configuration
#[tauri::command]
pub async fn update_ai_config(
    ai_config: AIConfig,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut config = state.config.write();
    config.ai = ai_config;

    // Save to disk
    crate::config::save_config(&config).map_err(|e| e.to_string())?;

    Ok(())
}

/// Test AI provider connection
#[tauri::command]
pub async fn test_ai_provider(
    provider_config: ProviderConfig,
    model: String,
) -> Result<String, String> {
    // Convert to agent types
    let agent_config = crate::agent::types::ProviderConfig::from(provider_config);
    test_provider(&agent_config, &model).await
}

// ============================================
// Task Template Commands (File-based)
// ============================================

/// Get all task templates from files
#[tauri::command]
pub async fn get_templates() -> Result<Vec<crate::templates::TemplateInfo>, String> {
    crate::templates::list_templates()
}

/// Get a single template by ID
#[tauri::command]
pub async fn get_template(id: String) -> Result<crate::templates::TemplateInfo, String> {
    crate::templates::get_template(&id)
}

/// Create or update a template
#[tauri::command]
pub async fn save_template(
    id: String,
    name: String,
    content: String,
    start_url: Option<String>,
    headless: bool,
) -> Result<(), String> {
    crate::templates::save_template(&id, &name, &content, start_url.as_deref(), headless)
}

/// Delete a template
#[tauri::command]
pub async fn delete_template(id: String) -> Result<(), String> {
    crate::templates::delete_template(&id)
}

/// Open the templates directory in file manager
#[tauri::command]
pub async fn open_templates_dir() -> Result<(), String> {
    crate::templates::open_templates_dir()
}

// ============================================
// Scheduled Task Commands
// ============================================

/// Get all scheduled tasks
#[tauri::command]
pub async fn get_scheduled_tasks(
    state: State<'_, AppState>,
) -> Result<Vec<crate::config::ScheduledTask>, String> {
    let config = state.config.read();
    Ok(config.scheduled_tasks.clone())
}

/// Add a new scheduled task
#[tauri::command]
pub async fn add_scheduled_task(
    task: crate::config::ScheduledTask,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut config = state.config.write();
    config.scheduled_tasks.push(task);

    // Save to disk
    crate::config::save_config(&config).map_err(|e| e.to_string())?;

    Ok(())
}

/// Update an existing scheduled task
#[tauri::command]
pub async fn update_scheduled_task(
    task: crate::config::ScheduledTask,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut config = state.config.write();

    if let Some(t) = config.scheduled_tasks.iter_mut().find(|t| t.id == task.id) {
        *t = task;
        // Save to disk
        crate::config::save_config(&config).map_err(|e| e.to_string())?;
        Ok(())
    } else {
        Err(format!("Scheduled task {} not found", task.id))
    }
}

/// Delete a scheduled task
#[tauri::command]
pub async fn delete_scheduled_task(
    task_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut config = state.config.write();
    let before_len = config.scheduled_tasks.len();
    config.scheduled_tasks.retain(|t| t.id != task_id);

    if config.scheduled_tasks.len() == before_len {
        return Err(format!("Scheduled task {} not found", task_id));
    }

    // Save to disk
    crate::config::save_config(&config).map_err(|e| e.to_string())?;

    Ok(())
}

/// Toggle scheduled task enabled state
#[tauri::command]
pub async fn toggle_scheduled_task(
    task_id: String,
    enabled: bool,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut config = state.config.write();

    if let Some(task) = config.scheduled_tasks.iter_mut().find(|t| t.id == task_id) {
        task.enabled = enabled;
        // Save to disk
        crate::config::save_config(&config).map_err(|e| e.to_string())?;
        Ok(())
    } else {
        Err(format!("Scheduled task {} not found", task_id))
    }
}
