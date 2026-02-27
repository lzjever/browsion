use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// How the app obtains the Chrome binary: CfT (default) or custom path (e.g. ungoogled).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BrowserSource {
    /// Use Chrome for Testing (official); download via in-app UX.
    ChromeForTesting {
        /// Release channel (default: Stable).
        #[serde(default = "default_cft_channel")]
        channel: CftChannel,
        /// Exact version string (e.g. "145.0.7632.117"). If None, use latest for channel.
        #[serde(skip_serializing_if = "Option::is_none")]
        version: Option<String>,
        /// Directory to download and extract CfT zips.
        #[serde(default = "default_cft_download_dir")]
        download_dir: PathBuf,
    },
    /// Use a custom Chrome/Chromium executable (e.g. ungoogled). Fingerprint/profile options apply.
    Custom {
        path: PathBuf,
        /// When true, this is adryfish/fingerprint-chromium; profile supports --fingerprint, --timezone, --lang.
        #[serde(default)]
        fingerprint_chromium: bool,
    },
}

fn default_cft_channel() -> CftChannel {
    CftChannel::Stable
}

fn default_cft_download_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".browsion")
        .join("cft")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "PascalCase")]
pub enum CftChannel {
    #[default]
    Stable,
    Beta,
    Dev,
    Canary,
}

impl CftChannel {
    pub fn as_str(&self) -> &'static str {
        match self {
            CftChannel::Stable => "Stable",
            CftChannel::Beta => "Beta",
            CftChannel::Dev => "Dev",
            CftChannel::Canary => "Canary",
        }
    }
}

impl Default for BrowserSource {
    fn default() -> Self {
        Self::ChromeForTesting {
            channel: CftChannel::Stable,
            version: None,
            download_dir: default_cft_download_dir(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// Where to get the Chrome binary: CfT (default) or custom path.
    #[serde(default)]
    pub browser_source: BrowserSource,

    /// Legacy: if present on load, migrated to browser_source = Custom(path). Not serialized.
    #[serde(skip_serializing, default)]
    pub chrome_path: Option<PathBuf>,

    /// Browser profile list
    #[serde(default)]
    pub profiles: Vec<BrowserProfile>,

    /// Application settings
    #[serde(default)]
    pub settings: AppSettings,

    /// Recently launched profile IDs (most recent first)
    #[serde(default)]
    pub recent_profiles: Vec<String>,

    /// MCP / API server configuration.
    #[serde(default)]
    pub mcp: McpConfig,

    /// Legacy: migrated into mcp.api_port on load. Not serialized.
    #[serde(skip_serializing, default)]
    pub api_port: Option<u16>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            browser_source: BrowserSource::default(),
            chrome_path: None,
            profiles: Vec::new(),
            settings: AppSettings::default(),
            recent_profiles: Vec::new(),
            mcp: McpConfig::default(),
            api_port: None,
        }
    }
}

/// MCP / API server configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpConfig {
    /// Whether the API server is enabled.
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Port for the local HTTP API. Default 38472.
    #[serde(default = "default_mcp_port")]
    pub api_port: u16,

    /// Optional API key. When set, all requests (except /api/health) must
    /// include `X-API-Key: <key>` header.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
}

fn default_mcp_port() -> u16 {
    38472
}

impl Default for McpConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            api_port: default_mcp_port(),
            api_key: None,
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

    /// Tags for categorization and filtering
    #[serde(default)]
    pub tags: Vec<String>,

    /// Launch Chrome in headless mode (no visible window). Default false.
    #[serde(default)]
    pub headless: bool,
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
    /// CDP remote-debugging port (if browser was launched with --remote-debugging-port).
    #[serde(default)]
    pub cdp_port: Option<u16>,
}

fn default_lang() -> String {
    "en-US".to_string()
}

fn default_true() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_config_mcp_defaults() {
        let config = AppConfig::default();
        assert!(config.mcp.enabled);
        assert_eq!(config.mcp.api_port, 38472);
        assert!(config.mcp.api_key.is_none());
    }

    #[test]
    fn test_browser_source_default_is_cft() {
        let source = BrowserSource::default();
        match &source {
            BrowserSource::ChromeForTesting { channel, version, .. } => {
                assert_eq!(*channel, CftChannel::Stable);
                assert!(version.is_none());
            }
            BrowserSource::Custom { .. } => panic!("default should be ChromeForTesting"),
        }
    }

    #[test]
    fn test_browser_source_custom_roundtrip() {
        let path = PathBuf::from("/tmp/custom-chrome");
        let source = BrowserSource::Custom {
            path: path.clone(),
            fingerprint_chromium: true,
        };
        let json = serde_json::to_string(&source).unwrap();
        assert!(json.contains("custom"));
        assert!(json.contains("/tmp/custom-chrome"));
        let decoded: BrowserSource = serde_json::from_str(&json).unwrap();
        match decoded {
            BrowserSource::Custom {
                path: p,
                fingerprint_chromium: fp,
            } => {
                assert_eq!(p, path);
                assert!(fp);
            }
            _ => panic!("expected Custom"),
        }
    }

    #[test]
    fn test_mcp_config_default() {
        let mcp = McpConfig::default();
        assert!(mcp.enabled);
        assert_eq!(mcp.api_port, 38472);
        assert!(mcp.api_key.is_none());
    }

    #[test]
    fn test_mcp_config_toml_roundtrip_with_key() {
        let toml = r#"
enabled = false
api_port = 9999
api_key = "sk-secret"
"#;
        let mcp: McpConfig = toml::from_str(toml).unwrap();
        assert!(!mcp.enabled);
        assert_eq!(mcp.api_port, 9999);
        assert_eq!(mcp.api_key.as_deref(), Some("sk-secret"));
        let back = toml::to_string(&mcp).unwrap();
        assert!(back.contains("9999"));
        assert!(back.contains("sk-secret"));
    }

    #[test]
    fn test_mcp_config_toml_roundtrip_without_key() {
        let toml = r#"
enabled = true
api_port = 38472
"#;
        let mcp: McpConfig = toml::from_str(toml).unwrap();
        assert!(mcp.enabled);
        assert_eq!(mcp.api_port, 38472);
        assert!(mcp.api_key.is_none());
    }
}
