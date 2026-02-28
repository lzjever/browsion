pub mod mcp_tools;
pub mod proxy;
pub mod recording;
pub mod snapshots;
pub mod workflow;
pub use mcp_tools::{detect_mcp_tools, find_mcp_binary, write_browsion_to_tool};
pub use proxy::{add_proxy_preset, delete_proxy_preset, get_proxy_presets, test_proxy, update_proxy_preset};
pub use recording::{delete_recording, get_recording, list_recordings, recording_to_workflow, save_recording};
pub use snapshots::{create_snapshot, delete_snapshot, list_snapshots, restore_snapshot};
pub use workflow::{delete_workflow, get_step_types, get_workflow, list_workflows, run_workflow, save_workflow, validate_workflow_step};

use crate::config::schema::BrowserSource;
use crate::config::{validation, BrowserProfile};
use crate::cft::{ensure_chrome_binary, fetch_versions, get_platform, CftProgress};
use crate::state::AppState;
use crate::window;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::Emitter;
use tauri::State;

/// Get all profiles
#[tauri::command]
pub async fn get_profiles(state: State<'_, Arc<AppState>>) -> Result<Vec<BrowserProfile>, String> {
    let config = state.config.read();
    Ok(config.profiles.clone())
}

/// Resolve effective Chrome path from config (CfT or custom). Used by commands and HTTP API.
pub async fn get_effective_chrome_path_from_config(
    config: &crate::config::AppConfig,
) -> Result<PathBuf, String> {
    match &config.browser_source {
        BrowserSource::Custom { path, .. } => {
            validation::validate_chrome_path(path).map_err(|e| e.to_string())?;
            Ok(path.clone())
        }
        BrowserSource::ChromeForTesting {
            channel,
            version,
            download_dir,
        } => {
            let platform = get_platform();
            let versions = fetch_versions(platform).await?;
            let channel_str = channel.as_str();
            let version_info = versions
                .iter()
                .find(|v| v.channel == channel_str)
                .ok_or_else(|| format!("Channel {} not found", channel_str))?;
            let version_info = if let Some(v) = version {
                versions
                    .iter()
                    .find(|i| i.version == *v)
                    .unwrap_or(version_info)
                    .clone()
            } else {
                version_info.clone()
            };
            ensure_chrome_binary(&version_info, download_dir, None).await
        }
    }
}

/// Resolve effective Chrome path from app state.
async fn get_effective_chrome_path(state: &State<'_, Arc<AppState>>) -> Result<PathBuf, String> {
    let config = state.config.read().clone();
    get_effective_chrome_path_from_config(&config).await
}

/// Get the current effective Chrome path (CfT binary or custom path).
#[tauri::command]
pub async fn get_chrome_path(state: State<'_, Arc<AppState>>) -> Result<String, String> {
    let path = get_effective_chrome_path(&state).await?;
    Ok(path.display().to_string())
}

/// Launch a profile. Returns PID (cdp_port is stored internally).
#[tauri::command]
pub async fn launch_profile(profile_id: String, state: State<'_, Arc<AppState>>) -> Result<u32, String> {
    let chrome_path = get_effective_chrome_path(&state).await?;
    let config = state.config.read().clone();
    let (pid, _cdp_port) = state
        .process_manager
        .launch_profile(&profile_id, &config, &chrome_path)
        .await
        .map_err(|e| e.to_string())?;

    {
        let mut config = state.config.write();
        config.recent_profiles.retain(|id| id != &profile_id);
        config.recent_profiles.insert(0, profile_id.clone());
        if config.recent_profiles.len() > 10 {
            config.recent_profiles.truncate(10);
        }
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
    state: State<'_, Arc<AppState>>,
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
pub async fn kill_profile(profile_id: String, state: State<'_, Arc<AppState>>) -> Result<(), String> {
    state
        .process_manager
        .kill_profile(&profile_id)
        .await
        .map_err(|e| e.to_string())
}

/// Get running status for all profiles
#[tauri::command]
pub async fn get_running_profiles(
    state: State<'_, Arc<AppState>>,
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
    state: State<'_, Arc<AppState>>,
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
    state: State<'_, Arc<AppState>>,
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
pub async fn delete_profile(profile_id: String, state: State<'_, Arc<AppState>>) -> Result<(), String> {
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

/// Set Chrome to use a custom executable path (e.g. ungoogled Chromium).
#[tauri::command]
pub async fn update_chrome_path(path: String, state: State<'_, Arc<AppState>>) -> Result<(), String> {
    let path_buf = PathBuf::from(&path);
    validation::validate_chrome_path(&path_buf).map_err(|e| e.to_string())?;

    let mut config = state.config.write();
    config.browser_source = BrowserSource::Custom {
        path: path_buf,
        fingerprint_chromium: false,
    };

    crate::config::save_config(&config).map_err(|e| e.to_string())?;
    Ok(())
}

/// Get browser source config (for UI: CfT channel/version or custom path).
#[tauri::command]
pub async fn get_browser_source(
    state: State<'_, Arc<AppState>>,
) -> Result<crate::config::schema::BrowserSource, String> {
    let config = state.config.read();
    Ok(config.browser_source.clone())
}

/// Update browser source (CfT channel/version/download_dir or custom path).
#[tauri::command]
pub async fn update_browser_source(
    source: crate::config::schema::BrowserSource,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    let mut config = state.config.write();
    config.browser_source = source;
    crate::config::save_config(&config).map_err(|e| e.to_string())?;
    Ok(())
}

/// Get available CfT versions for current platform.
#[tauri::command]
pub async fn get_cft_versions() -> Result<Vec<crate::cft::CftVersionInfo>, String> {
    let platform = get_platform();
    fetch_versions(platform).await
}

/// Download a CfT version and return the path to the Chrome binary.
/// Emits "cft-download-progress" events with CftProgress payload for UI progress bar.
#[tauri::command]
pub async fn download_cft_version(
    window: tauri::Window,
    channel: String,
    version: String,
    download_dir: Option<String>,
    state: State<'_, Arc<AppState>>,
) -> Result<String, String> {
    let platform = get_platform();
    let versions = fetch_versions(platform).await?;
    let version_info = versions
        .iter()
        .find(|v| v.channel == channel && v.version == version)
        .ok_or_else(|| format!("Version {} for channel {} not found", version, channel))?;

    let dir = match download_dir {
        Some(d) => PathBuf::from(d),
        None => {
            let config = state.config.read();
            match &config.browser_source {
                BrowserSource::ChromeForTesting { download_dir, .. } => download_dir.clone(),
                _ => default_cft_download_dir(),
            }
        }
    };

    let on_progress = Arc::new(move |p: CftProgress| {
        let _ = window.emit("cft-download-progress", &p);
    });

    let path = ensure_chrome_binary(version_info, &dir, Some(on_progress)).await?;
    Ok(path.display().to_string())
}

fn default_cft_download_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".browsion")
        .join("cft")
}

/// Get application settings
#[tauri::command]
pub async fn get_settings(
    state: State<'_, Arc<AppState>>,
) -> Result<crate::config::AppSettings, String> {
    let config = state.config.read();
    Ok(config.settings.clone())
}

/// Update application settings
#[tauri::command]
pub async fn update_settings(
    settings: crate::config::AppSettings,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    let mut config = state.config.write();
    config.settings = settings;

    // Save to disk
    crate::config::save_config(&config).map_err(|e| e.to_string())?;

    Ok(())
}

/// Get MCP / API server configuration
#[tauri::command]
pub async fn get_mcp_config(
    state: State<'_, Arc<AppState>>,
) -> Result<crate::config::schema::McpConfig, String> {
    let config = state.config.read();
    Ok(config.mcp.clone())
}

/// Update MCP / API server configuration.
/// Saves to disk, then stops and optionally restarts the HTTP API server.
#[tauri::command]
pub async fn update_mcp_config(
    mcp: crate::config::schema::McpConfig,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    {
        let mut config = state.config.write();
        config.mcp = mcp.clone();
        crate::config::save_config(&config).map_err(|e| e.to_string())?;
    }

    // Stop the existing server
    {
        let mut guard = state.api_server_abort.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(abort_fn) = guard.take() {
            abort_fn();
            tracing::info!("Stopped API server for reconfiguration");
        }
    }

    // Brief pause to let the old listener release the port
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Restart if enabled
    if mcp.enabled && mcp.api_port > 0 {
        let state_clone: Arc<AppState> = Arc::clone(&state);
        let api_key = mcp.api_key.clone();
        let port = mcp.api_port;
        let handle = tokio::spawn(async move {
            if let Err(e) = crate::api::run_server(state_clone, port, api_key).await {
                tracing::error!("API server error after restart: {}", e);
            }
        });
        let mut guard = state.api_server_abort.lock().unwrap_or_else(|e| e.into_inner());
        *guard = Some(Box::new(move || handle.abort()));
        tracing::info!("Restarted API server on port {}", mcp.api_port);
    }

    Ok(())
}

/// Get recently launched profiles
#[tauri::command]
pub async fn get_recent_profiles(
    state: State<'_, Arc<AppState>>,
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

