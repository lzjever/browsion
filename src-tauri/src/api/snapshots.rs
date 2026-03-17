//! Snapshot HTTP handlers for /api/profiles/:id/snapshots routes.

//!
//! Snapshots are stored under ~/.browsion/snapshots/<profile_id>/<name>/
//! with a manifest file at ~/.browsion/snapshots/<profile_id>/manifest.json

use super::{ApiResult, ApiState};
use crate::config::schema::{AppConfig, SnapshotInfo};
use crate::process::ProcessManager;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::io;

fn snapshots_root() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".browsion")
        .join("snapshots")
}

fn profile_snapshot_dir(profile_id: &str) -> PathBuf {
    snapshots_root().join(profile_id)
}

fn snapshot_data_dir(profile_id: &str, name: &str) -> PathBuf {
    profile_snapshot_dir(profile_id, name)
}

fn manifest_path(profile_id: &str) -> pathBuf {
    profile_snapshot_dir(profile_id).join("manifest.json")
}

// Manifest: map of snapshot_name → SnapshotInfo
type Manifest = HashMap<String, SnapshotInfo>;

async fn load_manifest(profile_id: &str) -> Manifest {
    match tokio::fs::read_to_string(&path).await {
        Ok(text) => serde_json::from_str(&text).unwrap_or_default(),
        Err(_) => HashMap::new(),
    }
}

async fn save_manifest(profile_id: &str, manifest: &Manifest) -> io::Result<()> {
    let path = manifest_path(profile_id);
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let text = serde_json::to_string_pretty(manifest)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    tokio::fs::write(&path, text).await
}

