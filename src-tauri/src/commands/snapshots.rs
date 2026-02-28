//! Profile snapshot: create / restore / list / delete browser profile data snapshots.
//!
//! Snapshots are stored under ~/.browsion/snapshots/<profile_id>/<name>/
//! with a manifest file at ~/.browsion/snapshots/<profile_id>/manifest.json

use crate::agent::SessionManager;
use crate::config::schema::{AppConfig, SnapshotInfo};
use crate::process::ProcessManager;
use crate::state::AppState;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::State;
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
    profile_snapshot_dir(profile_id).join(name)
}

fn manifest_path(profile_id: &str) -> PathBuf {
    profile_snapshot_dir(profile_id).join("manifest.json")
}

// Manifest: map of snapshot_name → SnapshotInfo
type Manifest = HashMap<String, SnapshotInfo>;

async fn load_manifest(profile_id: &str) -> Manifest {
    let path = manifest_path(profile_id);
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

/// Recursively compute total size of a directory.
async fn dir_size(path: &Path) -> u64 {
    let mut total = 0u64;
    let mut stack = vec![path.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let mut rd = match tokio::fs::read_dir(&dir).await {
            Ok(rd) => rd,
            Err(_) => continue,
        };
        while let Ok(Some(entry)) = rd.next_entry().await {
            let ft = match entry.file_type().await {
                Ok(ft) => ft,
                Err(_) => continue,
            };
            if ft.is_dir() {
                stack.push(entry.path());
            } else if let Ok(meta) = entry.metadata().await {
                total += meta.len();
            }
        }
    }
    total
}

/// Recursively copy src dir into dst dir (dst must not exist).
async fn copy_dir_all(src: PathBuf, dst: PathBuf) -> io::Result<()> {
    tokio::fs::create_dir_all(&dst).await?;
    let mut stack = vec![(src, dst)];
    while let Some((from, to)) = stack.pop() {
        let mut rd = tokio::fs::read_dir(&from).await?;
        while let Ok(Some(entry)) = rd.next_entry().await {
            let ft = entry.file_type().await?;
            let src_path = entry.path();
            let dst_path = to.join(entry.file_name());
            if ft.is_dir() {
                tokio::fs::create_dir_all(&dst_path).await?;
                stack.push((src_path, dst_path));
            } else {
                tokio::fs::copy(&src_path, &dst_path).await?;
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Core functions (shared by Tauri commands + HTTP handlers)
// ---------------------------------------------------------------------------

pub async fn core_list_snapshots(
    profile_id: &str,
    _config: &AppConfig,
) -> Result<Vec<SnapshotInfo>, String> {
    let manifest = load_manifest(profile_id).await;
    let mut infos: Vec<SnapshotInfo> = manifest.into_values().collect();
    infos.sort_by(|a, b| b.created_at_ts.cmp(&a.created_at_ts));
    Ok(infos)
}

pub async fn core_create_snapshot(
    profile_id: &str,
    name: &str,
    config: &AppConfig,
    process_mgr: &ProcessManager,
) -> Result<SnapshotInfo, String> {
    if process_mgr.is_running(profile_id) {
        return Err(format!(
            "Browser must be stopped before creating a snapshot for profile {}",
            profile_id
        ));
    }

    let profile = config
        .profiles
        .iter()
        .find(|p| p.id == profile_id)
        .ok_or_else(|| format!("Profile {} not found", profile_id))?;

    let src = profile.user_data_dir.clone();
    if !src.exists() {
        return Err(format!(
            "user_data_dir {:?} does not exist — launch and close the browser first",
            src
        ));
    }

    let dst = snapshot_data_dir(profile_id, name);
    if dst.exists() {
        return Err(format!("Snapshot '{}' already exists for profile {}", name, profile_id));
    }

    copy_dir_all(src.clone(), dst.clone())
        .await
        .map_err(|e| format!("Failed to copy profile data: {}", e))?;

    let size_bytes = dir_size(&dst).await;
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    let info = SnapshotInfo {
        name: name.to_string(),
        created_at_ts: ts,
        size_bytes,
    };

    let mut manifest = load_manifest(profile_id).await;
    manifest.insert(name.to_string(), info.clone());
    save_manifest(profile_id, &manifest)
        .await
        .map_err(|e| e.to_string())?;

    Ok(info)
}

pub async fn core_restore_snapshot(
    profile_id: &str,
    name: &str,
    config: &AppConfig,
    process_mgr: &ProcessManager,
    session_mgr: &SessionManager,
) -> Result<(), String> {
    if process_mgr.is_running(profile_id) {
        return Err(format!(
            "Kill the browser for profile {} before restoring a snapshot",
            profile_id
        ));
    }

    let profile = config
        .profiles
        .iter()
        .find(|p| p.id == profile_id)
        .ok_or_else(|| format!("Profile {} not found", profile_id))?;

    let snap_dir = snapshot_data_dir(profile_id, name);
    if !snap_dir.exists() {
        return Err(format!("Snapshot '{}' not found for profile {}", name, profile_id));
    }

    // Disconnect any open CDP session
    session_mgr.disconnect(profile_id).await;

    let dst = profile.user_data_dir.clone();

    // Remove existing user_data_dir
    if dst.exists() {
        tokio::fs::remove_dir_all(&dst)
            .await
            .map_err(|e| format!("Failed to remove existing user_data_dir: {}", e))?;
    }

    // Copy snapshot → user_data_dir
    copy_dir_all(snap_dir, dst)
        .await
        .map_err(|e| format!("Failed to restore snapshot: {}", e))?;

    Ok(())
}

pub async fn core_delete_snapshot(profile_id: &str, name: &str) -> Result<(), String> {
    let snap_dir = snapshot_data_dir(profile_id, name);
    if snap_dir.exists() {
        tokio::fs::remove_dir_all(&snap_dir)
            .await
            .map_err(|e| format!("Failed to delete snapshot directory: {}", e))?;
    }

    let mut manifest = load_manifest(profile_id).await;
    manifest.remove(name);
    save_manifest(profile_id, &manifest)
        .await
        .map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// Tauri commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn list_snapshots(
    profile_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<SnapshotInfo>, String> {
    let config = state.config.read().clone();
    core_list_snapshots(&profile_id, &config).await
}

#[tauri::command]
pub async fn create_snapshot(
    profile_id: String,
    name: String,
    state: State<'_, Arc<AppState>>,
) -> Result<SnapshotInfo, String> {
    let config = state.config.read().clone();
    core_create_snapshot(&profile_id, &name, &config, &state.process_manager).await
}

#[tauri::command]
pub async fn restore_snapshot(
    profile_id: String,
    name: String,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    let config = state.config.read().clone();
    core_restore_snapshot(
        &profile_id,
        &name,
        &config,
        &state.process_manager,
        &state.session_manager,
    )
    .await
}

#[tauri::command]
pub async fn delete_snapshot(
    profile_id: String,
    name: String,
    _state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    core_delete_snapshot(&profile_id, &name).await
}
