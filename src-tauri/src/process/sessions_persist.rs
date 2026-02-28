//! Persist running browser sessions across Tauri restarts.
//! Saved to ~/.browsion/running_sessions.json

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionEntry {
    pub pid: u32,
    pub cdp_port: u16,
}

fn sessions_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".browsion")
        .join("running_sessions.json")
}

/// Load persisted sessions map (profile_id → SessionEntry).
pub async fn load_sessions() -> io::Result<HashMap<String, SessionEntry>> {
    let path = sessions_path();
    match tokio::fs::read_to_string(&path).await {
        Ok(text) => serde_json::from_str(&text)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e)),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(HashMap::new()),
        Err(e) => Err(e),
    }
}

/// Upsert a session entry for the given profile.
pub async fn save_session(profile_id: &str, pid: u32, cdp_port: u16) -> io::Result<()> {
    let mut map = load_sessions().await.unwrap_or_default();
    map.insert(profile_id.to_string(), SessionEntry { pid, cdp_port });
    write_map(&map).await
}

/// Remove a session entry (called when browser is killed).
pub async fn remove_session(profile_id: &str) -> io::Result<()> {
    let mut map = load_sessions().await.unwrap_or_default();
    map.remove(profile_id);
    write_map(&map).await
}

async fn write_map(map: &HashMap<String, SessionEntry>) -> io::Result<()> {
    let path = sessions_path();
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let text = serde_json::to_string_pretty(map)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    tokio::fs::write(&path, text).await
}
