// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // Setup Linux IME (Input Method) support - must be before any GTK/WebKit init
    #[cfg(target_os = "linux")]
    {
        browsion_lib::platform::ime::setup_ime_env();
    }

    // Fix GBM buffer creation failure on some Linux systems
    #[cfg(target_os = "linux")]
    {
        if std::env::var("WEBKIT_DISABLE_COMPOSITING_MODE").is_err() {
            std::env::set_var("WEBKIT_DISABLE_COMPOSITING_MODE", "1");
        }
    }

    browsion_lib::run();
}
