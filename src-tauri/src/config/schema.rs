use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// Chrome executable path
    pub chrome_path: PathBuf,

    /// Browser profile list
    #[serde(default)]
    pub profiles: Vec<BrowserProfile>,

    /// Application settings
    #[serde(default)]
    pub settings: AppSettings,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            chrome_path: Self::default_chrome_path(),
            profiles: Vec::new(),
            settings: AppSettings::default(),
        }
    }
}

impl AppConfig {
    /// Get default Chrome path based on platform
    fn default_chrome_path() -> PathBuf {
        #[cfg(target_os = "windows")]
        {
            PathBuf::from("C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe")
        }
        #[cfg(target_os = "macos")]
        {
            PathBuf::from("/Applications/Google Chrome.app/Contents/MacOS/Google Chrome")
        }
        #[cfg(target_os = "linux")]
        {
            PathBuf::from("/usr/bin/google-chrome")
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserProfile {
    /// Unique identifier
    pub id: String,

    /// Display name
    pub name: String,

    /// Description
    #[serde(default)]
    pub description: String,

    /// User data directory
    pub user_data_dir: PathBuf,

    /// Proxy server (e.g., "http://192.168.0.220:8889")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proxy_server: Option<String>,

    /// Language
    #[serde(default = "default_lang")]
    pub lang: String,

    /// Timezone (e.g., "America/Los_Angeles")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timezone: Option<String>,

    /// Fingerprint ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fingerprint: Option<String>,

    /// UI color tag
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,

    /// Custom launch arguments
    #[serde(default)]
    pub custom_args: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    /// Auto start on system boot
    #[serde(default)]
    pub auto_start: bool,

    /// Minimize to tray on close
    #[serde(default = "default_true")]
    pub minimize_to_tray: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            auto_start: false,
            minimize_to_tray: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessInfo {
    pub profile_id: String,
    pub pid: u32,
    pub launched_at: u64, // Unix timestamp
}

fn default_lang() -> String {
    "en-US".to_string()
}

fn default_true() -> bool {
    true
}
