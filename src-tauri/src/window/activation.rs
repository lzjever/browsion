use crate::error::{BrowsionError, Result};

/// Activate (focus) a window by process ID
pub fn activate_window(pid: u32) -> Result<()> {
    #[cfg(target_os = "windows")]
    {
        activate_window_windows(pid)
    }

    #[cfg(target_os = "macos")]
    {
        activate_window_macos(pid)
    }

    #[cfg(target_os = "linux")]
    {
        activate_window_linux(pid)
    }
}

#[cfg(target_os = "windows")]
fn activate_window_windows(pid: u32) -> Result<()> {
    use windows::Win32::Foundation::{BOOL, HWND, LPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindowThreadProcessId, IsWindowVisible, SetForegroundWindow, ShowWindow,
        SW_RESTORE,
    };

    unsafe {
        let mut target_hwnd: HWND = HWND(0);
        let pid_to_find = pid;

        // Callback to find window by PID
        unsafe extern "system" fn enum_windows_callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
            let target_pid = lparam.0 as u32;
            let mut window_pid: u32 = 0;

            GetWindowThreadProcessId(hwnd, Some(&mut window_pid));

            if window_pid == target_pid && IsWindowVisible(hwnd).as_bool() {
                // Found the window, store it
                let target_hwnd_ptr = lparam.0 as *mut HWND;
                *target_hwnd_ptr = hwnd;
                return BOOL(0); // Stop enumeration
            }

            BOOL(1) // Continue enumeration
        }

        // Find the window
        let result = EnumWindows(
            Some(enum_windows_callback),
            LPARAM(&mut target_hwnd as *mut HWND as isize),
        );

        if target_hwnd.0 == 0 {
            return Err(BrowsionError::Window(format!(
                "No window found for PID {}",
                pid
            )));
        }

        // Restore and bring to foreground
        ShowWindow(target_hwnd, SW_RESTORE);
        SetForegroundWindow(target_hwnd).ok().map_err(|e| {
            BrowsionError::Window(format!("Failed to set foreground window: {:?}", e))
        })?;

        tracing::info!("Activated window for PID {}", pid);
        Ok(())
    }
}

#[cfg(target_os = "macos")]
fn activate_window_macos(pid: u32) -> Result<()> {
    use cocoa::appkit::NSRunningApplication;
    use cocoa::base::nil;
    use cocoa::foundation::NSInteger;
    use objc::{class, msg_send, sel, sel_impl};

    unsafe {
        let app: *mut objc::runtime::Object = msg_send![
            class!(NSRunningApplication),
            runningApplicationWithProcessIdentifier: pid as NSInteger
        ];

        if app == nil {
            return Err(BrowsionError::Window(format!(
                "No application found for PID {}",
                pid
            )));
        }

        // NSApplicationActivateIgnoringOtherApps = 1 << 1
        let _: () = msg_send![app, activateWithOptions: 1 << 1];

        tracing::info!("Activated window for PID {}", pid);
        Ok(())
    }
}

#[cfg(target_os = "linux")]
fn activate_window_linux(pid: u32) -> Result<()> {
    // Try xdotool first (works best with PID)
    let result = std::process::Command::new("xdotool")
        .args([
            "search",
            "--pid",
            &format!("{}", pid),
            "windowactivate",
            "%@",
        ])
        .output();

    match result {
        Ok(output) if output.status.success() => {
            tracing::info!("Activated window for PID {} using xdotool", pid);
            return Ok(());
        }
        Ok(output) => {
            tracing::warn!(
                "xdotool failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        Err(e) => {
            tracing::warn!("xdotool not available: {}", e);
        }
    }

    // Fallback: Try wmctrl with window listing and PID matching
    // First, list all windows and find the one with our PID
    let list_result = std::process::Command::new("wmctrl")
        .args(["-l", "-p"])
        .output();

    if let Ok(list_output) = list_result {
        if list_output.status.success() {
            let output_str = String::from_utf8_lossy(&list_output.stdout);
            // Format: 0x03a00003  0 19283  hostname window-title
            for line in output_str.lines() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 3 {
                    if let Ok(window_pid) = parts[2].parse::<u32>() {
                        if window_pid == pid {
                            // Found the window! Try to activate by window ID
                            let window_id = parts[0];
                            let activate_result = std::process::Command::new("wmctrl")
                                .args(["-i", "-a", window_id])
                                .output();

                            if let Ok(activate_output) = activate_result {
                                if activate_output.status.success() {
                                    tracing::info!(
                                        "Activated window {} for PID {} using wmctrl",
                                        window_id,
                                        pid
                                    );
                                    return Ok(());
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Final fallback error
    Err(BrowsionError::Window(format!(
        "Failed to activate window for PID {}. Please install xdotool (recommended) or wmctrl.",
        pid
    )))
}
