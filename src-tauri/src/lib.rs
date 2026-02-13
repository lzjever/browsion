pub mod commands;
pub mod config;
pub mod error;
pub mod process;
pub mod state;
pub mod tray;
pub mod window;

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
            // Load configuration
            let config = config::load_config().unwrap_or_else(|e| {
                tracing::error!("Failed to load config: {}", e);
                config::AppConfig::default()
            });

            // Create application state
            let state = AppState::new(config);

            // Manage state
            app.manage(state);

            // Setup system tray
            tray::setup_tray(app.handle())?;

            // Start background task to cleanup dead processes
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(2));
                loop {
                    interval.tick().await;
                    if let Some(state) = app_handle.try_state::<AppState>() {
                        if let Err(e) = state.process_manager.cleanup_dead_processes().await {
                            tracing::error!("Failed to cleanup dead processes: {}", e);
                        }
                    }
                }
            });

            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                // Get settings to check minimize_to_tray
                let state = window.state::<AppState>();
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
            commands::get_settings,
            commands::update_settings,
            commands::get_recent_profiles,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
