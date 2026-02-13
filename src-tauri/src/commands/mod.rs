use crate::config::{validation, BrowserProfile};
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
