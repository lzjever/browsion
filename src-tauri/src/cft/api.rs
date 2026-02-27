//! CfT JSON API types and fetch.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

const VERSIONS_URL: &str =
    "https://googlechromelabs.github.io/chrome-for-testing/last-known-good-versions-with-downloads.json";

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct LastKnownGoodVersions {
    pub timestamp: Option<String>,
    pub channels: HashMap<String, ChannelInfo>,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct ChannelInfo {
    pub channel: String,
    pub version: String,
    #[serde(default)]
    pub revision: Option<String>,
    pub downloads: Option<Downloads>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Downloads {
    pub chrome: Option<Vec<DownloadItem>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DownloadItem {
    pub platform: String,
    pub url: String,
}

/// Channel name as in the API (PascalCase).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CftChannelApi {
    Stable,
    Beta,
    Dev,
    Canary,
}

impl CftChannelApi {
    pub fn as_str(&self) -> &'static str {
        match self {
            CftChannelApi::Stable => "Stable",
            CftChannelApi::Beta => "Beta",
            CftChannelApi::Dev => "Dev",
            CftChannelApi::Canary => "Canary",
        }
    }
}

/// Version info for one channel, with download URL for current platform.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CftVersionInfo {
    pub channel: String,
    pub version: String,
    pub url: String,
    pub platform: String,
}

/// Fetch last-known-good-versions-with-downloads and return version info for each channel for the given platform.
pub async fn fetch_versions(platform: &str) -> Result<Vec<CftVersionInfo>, String> {
    let body = reqwest::get(VERSIONS_URL)
        .await
        .map_err(|e| format!("Failed to fetch CfT versions: {}", e))?
        .error_for_status()
        .map_err(|e| format!("CfT API error: {}", e))?
        .text()
        .await
        .map_err(|e| format!("Failed to read response: {}", e))?;

    let data: LastKnownGoodVersions =
        serde_json::from_str(&body).map_err(|e| format!("Invalid CfT JSON: {}", e))?;

    let mut out = Vec::new();
    for (channel_name, info) in &data.channels {
        let url = info
            .downloads
            .as_ref()
            .and_then(|d| d.chrome.as_ref())
            .and_then(|items| items.iter().find(|i| i.platform == platform))
            .map(|i| i.url.clone())
            .ok_or_else(|| format!("No chrome download for platform {} in channel {}", platform, channel_name))?;
        out.push(CftVersionInfo {
            channel: channel_name.clone(),
            version: info.version.clone(),
            url,
            platform: platform.to_string(),
        });
    }
    Ok(out)
}
