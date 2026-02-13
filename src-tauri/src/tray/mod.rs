use crate::state::AppState;
use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem, Submenu},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager, Runtime,
};

/// Setup system tray with dynamic menu
pub fn setup_tray<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let menu = build_tray_menu(app)?;

    // Build tray icon
    let _tray = TrayIconBuilder::new()
        .icon(app.default_window_icon().unwrap().clone())
        .menu(&menu)
        .on_menu_event(move |app, event| {
            let event_id = event.id.as_ref();

            if event_id == "show" {
                show_main_window(app);
            } else if event_id == "quit" {
                app.exit(0);
            } else if event_id.starts_with("profile_") {
                // Handle profile menu click
                let profile_id = event_id.strip_prefix("profile_").unwrap();
                handle_profile_click(app, profile_id);
            }
        })
        .on_tray_icon_event(|tray, event| {
            match event {
                // Double-click to show window
                TrayIconEvent::DoubleClick {
                    button: MouseButton::Left,
                    ..
                } => {
                    show_main_window(tray.app_handle());
                }
                // Single left-click also shows window
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                } => {
                    show_main_window(tray.app_handle());
                }
                _ => {}
            }
        })
        .build(app)?;

    Ok(())
}

/// Build the tray menu with recent profiles
fn build_tray_menu<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Menu<R>> {
    let show_item = MenuItem::with_id(app, "show", "Show Window", true, None::<&str>)?;

    // Get recent profiles from state
    let mut menu_items: Vec<Box<dyn tauri::menu::IsMenuItem<R>>> = vec![Box::new(show_item)];

    if let Some(state) = app.try_state::<AppState>() {
        let config = state.config.read();
        let recent_ids = &config.recent_profiles;

        if !recent_ids.is_empty() {
            // Add separator
            let separator = PredefinedMenuItem::separator(app)?;
            menu_items.push(Box::new(separator));

            // Add recent profiles submenu
            let mut recent_submenu_items: Vec<Box<dyn tauri::menu::IsMenuItem<R>>> = Vec::new();

            for (i, profile_id) in recent_ids.iter().take(10).enumerate() {
                if let Some(profile) = config.profiles.iter().find(|p| &p.id == profile_id) {
                    let is_running = state.process_manager.is_running(profile_id);
                    let status = if is_running { "●" } else { "○" };
                    let label = format!("{} {}", status, profile.name);

                    let item = MenuItem::with_id(
                        app,
                        format!("profile_{}", profile_id),
                        label,
                        true,
                        None::<&str>,
                    )?;
                    recent_submenu_items.push(Box::new(item));

                    if i >= 9 {
                        break;
                    }
                }
            }

            if !recent_submenu_items.is_empty() {
                let recent_submenu = Submenu::with_items(
                    app,
                    "Recent Profiles",
                    true,
                    &recent_submenu_items
                        .iter()
                        .map(|item| item.as_ref())
                        .collect::<Vec<_>>(),
                )?;
                menu_items.push(Box::new(recent_submenu));
            }
        }
    }

    // Add separator and quit
    let separator = PredefinedMenuItem::separator(app)?;
    menu_items.push(Box::new(separator));

    let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    menu_items.push(Box::new(quit_item));

    // Build final menu
    Menu::with_items(
        app,
        &menu_items
            .iter()
            .map(|item| item.as_ref())
            .collect::<Vec<_>>(),
    )
}

/// Show and focus the main window
fn show_main_window<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window("main") {
        // Check if window is minimized and unminimize it
        if let Ok(is_minimized) = window.is_minimized() {
            if is_minimized {
                let _ = window.unminimize();
            }
        }

        // Show and focus the window
        let _ = window.show();
        let _ = window.set_focus();

        // On Linux, might need extra activation
        #[cfg(target_os = "linux")]
        {
            let _ = window.set_focus();
        }
    }
}

/// Handle profile click from tray menu
fn handle_profile_click<R: Runtime>(app: &AppHandle<R>, profile_id: &str) {
    if let Some(state) = app.try_state::<AppState>() {
        let is_running = state.process_manager.is_running(profile_id);

        if is_running {
            // Activate existing window
            if let Some(info) = state.process_manager.get_process_info(profile_id) {
                if let Err(e) = crate::window::activate_window(info.pid) {
                    tracing::error!(
                        "Failed to activate window for profile {}: {}",
                        profile_id,
                        e
                    );
                }
            }
        } else {
            // Launch new instance
            let config = state.config.read().clone();
            let process_manager = state.process_manager.clone();
            let profile_id = profile_id.to_string();

            tauri::async_runtime::spawn(async move {
                if let Err(e) = process_manager.launch_profile(&profile_id, &config).await {
                    tracing::error!("Failed to launch profile {}: {}", profile_id, e);
                }
            });
        }
    }
}

/// Update tray menu (called when profiles change)
pub fn update_tray_menu<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    // Rebuild the tray with updated menu
    // Note: This is a simplified version. In production, you might want to
    // keep a handle to the tray and update only the menu
    setup_tray(app)
}
