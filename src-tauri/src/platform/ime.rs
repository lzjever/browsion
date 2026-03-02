//! Linux IME (Input Method Editor) support.
//!
//! On Linux, especially with Wayland, `~/.xprofile` may not be read, causing
//! IME environment variables to be missing. This module detects the active
//! IME framework and sets up the required environment variables.

use std::process::Command;

/// Supported IME frameworks on Linux.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImeFramework {
    Fcitx5,
    Fcitx,
    Ibus,
    None,
}

impl ImeFramework {
    /// Returns the module name for GTK_IM_MODULE and QT_IM_MODULE.
    pub fn module_name(&self) -> Option<&'static str> {
        match self {
            ImeFramework::Fcitx5 => Some("fcitx"),
            ImeFramework::Fcitx => Some("fcitx"),
            ImeFramework::Ibus => Some("ibus"),
            ImeFramework::None => None,
        }
    }

    /// Returns the XMODIFIERS value (@im=xxx).
    pub fn xmodifiers(&self) -> Option<&'static str> {
        match self {
            ImeFramework::Fcitx5 => Some("@im=fcitx"),
            ImeFramework::Fcitx => Some("@im=fcitx"),
            ImeFramework::Ibus => Some("@im=ibus"),
            ImeFramework::None => None,
        }
    }
}

/// Detect the active IME framework.
///
/// Detection order:
/// 1. Check for running processes (pgrep)
/// 2. Check existing environment variables
/// 3. Check for binary existence
pub fn detect_ime_framework() -> ImeFramework {
    // 1. Check running processes
    if is_process_running("fcitx5") {
        return ImeFramework::Fcitx5;
    }
    if is_process_running("fcitx") {
        return ImeFramework::Fcitx;
    }
    if is_process_running("ibus-daemon") {
        return ImeFramework::Ibus;
    }

    // 2. Check existing environment variables
    if let Ok(gtk_im) = std::env::var("GTK_IM_MODULE") {
        if gtk_im == "fcitx" {
            // Could be fcitx or fcitx5, check binary
            if which("fcitx5") {
                return ImeFramework::Fcitx5;
            }
            return ImeFramework::Fcitx;
        }
        if gtk_im == "ibus" {
            return ImeFramework::Ibus;
        }
    }

    // 3. Check binary existence
    if which("fcitx5") {
        return ImeFramework::Fcitx5;
    }
    if which("fcitx") {
        return ImeFramework::Fcitx;
    }
    if which("ibus-daemon") {
        return ImeFramework::Ibus;
    }

    ImeFramework::None
}

/// Check if a process is running using pgrep.
fn is_process_running(name: &str) -> bool {
    Command::new("pgrep")
        .arg("-x")
        .arg(name)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Check if a binary exists in PATH.
fn which(name: &str) -> bool {
    Command::new("which")
        .arg(name)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Set up IME environment variables if not already set.
///
/// This should be called as early as possible in main(), before any
/// GTK/WebKit initialization.
pub fn setup_ime_env() {
    let framework = detect_ime_framework();

    if framework == ImeFramework::None {
        tracing::debug!("No IME framework detected");
        return;
    }

    tracing::info!("Detected IME framework: {:?}", framework);

    // Set GTK_IM_MODULE (for WebKitGTK and Chrome GTK integration)
    if std::env::var("GTK_IM_MODULE").is_err() {
        if let Some(module) = framework.module_name() {
            std::env::set_var("GTK_IM_MODULE", module);
            tracing::debug!("Set GTK_IM_MODULE={}", module);
        }
    }

    // Set QT_IM_MODULE (for Qt applications)
    if std::env::var("QT_IM_MODULE").is_err() {
        if let Some(module) = framework.module_name() {
            std::env::set_var("QT_IM_MODULE", module);
            tracing::debug!("Set QT_IM_MODULE={}", module);
        }
    }

    // Set XMODIFIERS (for X11 compatibility layer)
    if std::env::var("XMODIFIERS").is_err() {
        if let Some(xmod) = framework.xmodifiers() {
            std::env::set_var("XMODIFIERS", xmod);
            tracing::debug!("Set XMODIFIERS={}", xmod);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ime_framework_module_name() {
        assert_eq!(ImeFramework::Fcitx5.module_name(), Some("fcitx"));
        assert_eq!(ImeFramework::Fcitx.module_name(), Some("fcitx"));
        assert_eq!(ImeFramework::Ibus.module_name(), Some("ibus"));
        assert_eq!(ImeFramework::None.module_name(), None);
    }

    #[test]
    fn test_ime_framework_xmodifiers() {
        assert_eq!(ImeFramework::Fcitx5.xmodifiers(), Some("@im=fcitx"));
        assert_eq!(ImeFramework::Fcitx.xmodifiers(), Some("@im=fcitx"));
        assert_eq!(ImeFramework::Ibus.xmodifiers(), Some("@im=ibus"));
        assert_eq!(ImeFramework::None.xmodifiers(), None);
    }

    #[test]
    fn test_detect_ime_framework_returns_none_without_ime() {
        // This test just ensures the function runs without crashing
        // The actual result depends on the system
        let _framework = detect_ime_framework();
    }
}
