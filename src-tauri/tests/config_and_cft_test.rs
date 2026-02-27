//! Tests for config (browser_source, default), CfT module, and effective path resolution.

use browsion_lib::config::schema::{AppConfig, BrowserSource, CftChannel};
use browsion_lib::cft::get_platform;
use std::path::PathBuf;

#[test]
fn test_app_config_default_uses_cft() {
    let config = AppConfig::default();
    assert!(config.chrome_path.is_none());
    match &config.browser_source {
        BrowserSource::ChromeForTesting { channel, version, .. } => {
            assert_eq!(*channel, CftChannel::Stable);
            assert!(version.is_none());
        }
        BrowserSource::Custom { .. } => panic!("default should be ChromeForTesting"),
    }
}

#[test]
fn test_cft_channel_as_str() {
    assert_eq!(CftChannel::Stable.as_str(), "Stable");
    assert_eq!(CftChannel::Beta.as_str(), "Beta");
    assert_eq!(CftChannel::Dev.as_str(), "Dev");
    assert_eq!(CftChannel::Canary.as_str(), "Canary");
}

#[test]
fn test_get_platform_returns_known_platform() {
    let platform = get_platform();
    let known = ["linux64", "mac-arm64", "mac-x64", "win32", "win64"];
    assert!(
        known.contains(&platform),
        "get_platform should return one of {:?}, got {}",
        known,
        platform
    );
}

#[tokio::test]
async fn test_effective_chrome_path_custom_missing_fails() {
    let config = AppConfig {
        browser_source: BrowserSource::Custom {
            path: PathBuf::from("/nonexistent/path/to/chrome"),
            fingerprint_chromium: false,
        },
        ..AppConfig::default()
    };
    let result = browsion_lib::commands::get_effective_chrome_path_from_config(&config).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.contains("not found") || err.contains("Chrome"));
}

#[test]
fn test_cft_versions_json_parsing() {
    let sample = r#"{
        "timestamp": "2025-01-01T00:00:00.000Z",
        "channels": {
            "Stable": {
                "channel": "Stable",
                "version": "120.0.6099.0",
                "revision": "123456",
                "downloads": {
                    "chrome": [
                        {"platform": "linux64", "url": "https://example.com/linux64.zip"}
                    ]
                }
            }
        }
    }"#;
    let data: browsion_lib::cft::LastKnownGoodVersions =
        serde_json::from_str(sample).unwrap();
    assert!(data.channels.get("Stable").is_some());
    let stable = data.channels.get("Stable").unwrap();
    assert_eq!(stable.version, "120.0.6099.0");
    let url = stable
        .downloads
        .as_ref()
        .and_then(|d| d.chrome.as_ref())
        .and_then(|c| c.first())
        .map(|i| i.url.as_str())
        .unwrap();
    assert_eq!(url, "https://example.com/linux64.zip");
}

#[tokio::test]
async fn test_effective_chrome_path_custom_valid_succeeds() {
    // Use current executable as a valid path that exists
    let current_exe = std::env::current_exe().unwrap();
    let config = AppConfig {
        browser_source: BrowserSource::Custom {
            path: current_exe.clone(),
            fingerprint_chromium: false,
        },
        ..AppConfig::default()
    };
    let result = browsion_lib::commands::get_effective_chrome_path_from_config(&config).await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), current_exe);
}
