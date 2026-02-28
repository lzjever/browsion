//! Tauri commands for proxy preset management.

use crate::config::schema::ProxyPreset;
use crate::state::AppState;
use std::sync::Arc;
use tauri::State;

/// Get all proxy presets.
#[tauri::command]
pub async fn get_proxy_presets(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<ProxyPreset>, String> {
    let config = state.config.read();
    Ok(config.proxy_presets.clone())
}

/// Add a new proxy preset. Generates a UUID id.
#[tauri::command]
pub async fn add_proxy_preset(
    name: String,
    url: String,
    state: State<'_, Arc<AppState>>,
) -> Result<ProxyPreset, String> {
    let preset = ProxyPreset {
        id: uuid::Uuid::new_v4().to_string(),
        name,
        url,
    };
    {
        let mut config = state.config.write();
        config.proxy_presets.push(preset.clone());
        crate::config::save_config(&config).map_err(|e| e.to_string())?;
    }
    Ok(preset)
}

/// Update an existing proxy preset by id.
#[tauri::command]
pub async fn update_proxy_preset(
    preset: ProxyPreset,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    let mut config = state.config.write();
    if let Some(p) = config.proxy_presets.iter_mut().find(|p| p.id == preset.id) {
        *p = preset;
        crate::config::save_config(&config).map_err(|e| e.to_string())?;
        Ok(())
    } else {
        Err(format!("Proxy preset {} not found", preset.id))
    }
}

/// Delete a proxy preset by id.
#[tauri::command]
pub async fn delete_proxy_preset(
    id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    let mut config = state.config.write();
    let before = config.proxy_presets.len();
    config.proxy_presets.retain(|p| p.id != id);
    if config.proxy_presets.len() == before {
        return Err(format!("Proxy preset {} not found", id));
    }
    crate::config::save_config(&config).map_err(|e| e.to_string())
}

/// Test a proxy by timing a GET to https://example.com through it.
/// Returns latency in ms on success or an error message.
#[tauri::command]
pub async fn test_proxy(url: String) -> Result<u64, String> {
    use std::time::Instant;

    let proxy = reqwest::Proxy::all(&url).map_err(|e| format!("Invalid proxy URL: {}", e))?;
    let client = reqwest::Client::builder()
        .proxy(proxy)
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())?;

    let t0 = Instant::now();
    client
        .get("https://example.com")
        .send()
        .await
        .map_err(|e| format!("Proxy test failed: {}", e))?;

    Ok(t0.elapsed().as_millis() as u64)
}
