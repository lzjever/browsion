pub mod agent;
pub mod api;
pub mod commands;
pub mod config;
pub mod cft;
pub mod error;
pub mod platform;
pub mod process;
pub mod recording;
pub mod state;
pub mod tray;
pub mod window;
pub mod workflow;

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

            // Inject app handle so HTTP API handlers can emit events to frontend
            *state.app_handle.lock() = Some(app.handle().clone());

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

            // Session reconnect: probe previously-running browsers from saved sessions
            {
                let state_rc = std::sync::Arc::clone(&state);
                tauri::async_runtime::spawn(async move {
                    match crate::process::sessions_persist::load_sessions().await {
                        Ok(sessions) => {
                            for (profile_id, entry) in sessions {
                                // Probe CDP port to check if browser is still alive
                                let url = format!("http://127.0.0.1:{}/json/version", entry.cdp_port);
                                match reqwest::get(&url).await {
                                    Ok(r) if r.status().is_success() => {
                                        tracing::info!(
                                            "Reconnected session: profile={} pid={} cdp_port={}",
                                            profile_id,
                                            entry.pid,
                                            entry.cdp_port
                                        );
                                        state_rc.process_manager.register_external(
                                            &profile_id,
                                            entry.pid,
                                            entry.cdp_port,
                                        );
                                    }
                                    _ => {
                                        tracing::info!(
                                            "Session dead on restart, removing: profile={}",
                                            profile_id
                                        );
                                        let _ = crate::process::sessions_persist::remove_session(
                                            &profile_id,
                                        )
                                        .await;
                                    }
                                }
                            }
                            state_rc.emit("browser-status-changed");
                        }
                        Err(e) => tracing::warn!("Failed to load persisted sessions: {}", e),
                    }
                });
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
                                state.emit("browser-status-changed");
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
            commands::mcp_tools::detect_mcp_tools,
            commands::mcp_tools::write_browsion_to_tool,
            commands::mcp_tools::find_mcp_binary,
            commands::proxy::get_proxy_presets,
            commands::proxy::add_proxy_preset,
            commands::proxy::update_proxy_preset,
            commands::proxy::delete_proxy_preset,
            commands::proxy::test_proxy,
            commands::snapshots::list_snapshots,
            commands::snapshots::create_snapshot,
            commands::snapshots::restore_snapshot,
            commands::snapshots::delete_snapshot,
            commands::workflow::list_workflows,
            commands::workflow::get_workflow,
            commands::workflow::save_workflow,
            commands::workflow::delete_workflow,
            commands::workflow::run_workflow,
            commands::workflow::validate_workflow_step,
            commands::workflow::get_step_types,
            commands::recording::list_recordings,
            commands::recording::get_recording,
            commands::recording::save_recording,
            commands::recording::delete_recording,
            commands::recording::recording_to_workflow,
            commands::recording::start_recording,
            commands::recording::stop_recording,
            commands::recording::get_active_recording_sessions,
            commands::recording::is_recording,
            commands::recording::get_recording_session_info,
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
