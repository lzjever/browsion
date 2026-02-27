//! Chrome for Testing (CfT) integration.
//! Fetches version list from official JSON API, downloads and extracts Chrome binary.

mod api;
mod download;

pub use api::{fetch_versions, CftChannelApi, CftVersionInfo, LastKnownGoodVersions};
pub use download::{ensure_chrome_binary, get_platform, CftProgress};
