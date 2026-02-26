pub mod agent;
pub mod api;
pub mod commands;
pub mod config;
pub mod cft;
pub mod error;
pub mod process;
pub mod state;
pub mod tray;
pub mod window;

use std::sync::Arc;
use state::AppState;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            // Load configuration (never overwrite existing file on failure)
            let config = match config::load_config() {
                Ok(c) => c,
                Err(e) => {
                    tracing::error!("Failed to load config: {}. Using in-memory defaults (not saving).", e);
                    config::AppConfig::default()
                }
            };

            // Create application state (Arc so API server can share it)
            let state = std::sync::Arc::new(AppState::new(config.clone()));

            // Start local HTTP API if MCP config says enabled
            let mcp = &config.mcp;
            if mcp.enabled && mcp.api_port > 0 {
                let state_clone = std::sync::Arc::clone(&state);
                let port = mcp.api_port;
                let api_key = mcp.api_key.clone();
                let handle = tauri::async_runtime::spawn(async move {
                    if let Err(e) = crate::api::run_server(state_clone, port, api_key).await {
                        tracing::error!("API server error: {}", e);
                    }
                });
                if let Ok(mut guard) = state.api_server_abort.lock() {
                    *guard = Some(Box::new(move || handle.abort()));
                }
            }

            // Manage state
            app.manage(state);

            // Setup system tray
            tray::setup_tray(app.handle())?;

            // Background task: cleanup dead processes + stale CDP sessions every 30s
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(30));
                loop {
                    interval.tick().await;
                    if let Some(state) = app_handle.try_state::<Arc<AppState>>() {
                        match state.process_manager.cleanup_dead_processes().await {
                            Ok(removed) if !removed.is_empty() => {
                                tracing::info!("Auto-cleaned dead processes: {:?}", removed);
                                for profile_id in &removed {
                                    state.session_manager.disconnect(profile_id).await;
                                }
                            }
                            Ok(_) => {}
                            Err(e) => tracing::warn!("Dead process cleanup failed: {}", e),
                        }
                    }
                }
            });

            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                // Get settings to check minimize_to_tray
                let state = window.state::<Arc<AppState>>();
                let config = state.config.read();
                if config.settings.minimize_to_tray {
                    // Hide window instead of closing
                    window.hide().unwrap();
                    api.prevent_close();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_profiles,
            commands::get_chrome_path,
            commands::launch_profile,
            commands::activate_profile,
            commands::kill_profile,
            commands::get_running_profiles,
            commands::add_profile,
            commands::update_profile,
            commands::delete_profile,
            commands::update_chrome_path,
            commands::get_browser_source,
            commands::update_browser_source,
            commands::get_cft_versions,
            commands::download_cft_version,
            commands::get_settings,
            commands::update_settings,
            commands::get_recent_profiles,
            commands::get_mcp_config,
            commands::update_mcp_config,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod mcp_command_tests {
    use super::*;
    use crate::commands::{get_mcp_config, update_mcp_config};
    use crate::config::schema::McpConfig;
    use crate::config::AppConfig;
    use std::sync::Arc;
    use tauri::test::{assert_ipc_response, get_ipc_response, mock_builder, mock_context, noop_assets, INVOKE_KEY};
    use tauri::WebviewWindowBuilder;

    fn create_app_with_mcp_state(config: AppConfig) -> tauri::App<tauri::test::MockRuntime> {
        let state = Arc::new(AppState::new(config));
        mock_builder()
            .manage(state)
            .invoke_handler(tauri::generate_handler![get_mcp_config, update_mcp_config])
            .build(mock_context(noop_assets()))
            .expect("failed to build app")
    }

    fn invoke_request(cmd: &str, body: serde_json::Value) -> tauri::webview::InvokeRequest {
        tauri::webview::InvokeRequest {
            cmd: cmd.to_string(),
            callback: tauri::ipc::CallbackFn(0),
            error: tauri::ipc::CallbackFn(1),
            url: "http://tauri.localhost".parse().unwrap(),
            body: tauri::ipc::InvokeBody::Json(body),
            headers: Default::default(),
            invoke_key: INVOKE_KEY.to_string(),
        }
    }

    #[test]
    fn test_get_mcp_config_returns_default() {
        let app = create_app_with_mcp_state(AppConfig::default());
        let webview = WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .unwrap();

        let req = invoke_request("get_mcp_config", serde_json::json!({}));
        let expected = McpConfig::default();
        assert_ipc_response(&webview, req, Ok(expected));
    }

    #[test]
    fn test_get_mcp_config_deserialize_response() {
        let app = create_app_with_mcp_state(AppConfig::default());
        let webview = WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .unwrap();

        let req = invoke_request("get_mcp_config", serde_json::json!({}));
        let res = get_ipc_response(&webview, req);
        assert!(res.is_ok());
        let body = res.unwrap();
        let mcp: McpConfig = body.deserialize().unwrap();
        assert!(mcp.enabled);
        assert_eq!(mcp.api_port, 38472);
        assert!(mcp.api_key.is_none());
    }
}
