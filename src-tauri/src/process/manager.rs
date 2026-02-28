use crate::config::schema::{AppConfig, ProcessInfo};
use crate::error::{BrowsionError, Result};
use crate::process::launcher;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use sysinfo::{Pid, ProcessRefreshKind, System};

pub struct ProcessManager {
    /// Map of profile_id -> ProcessInfo
    active_processes: Arc<Mutex<HashMap<String, ProcessInfo>>>,
    /// System info for process tracking
    system: Arc<Mutex<System>>,
    /// Recently launched profiles (most recent first)
    recent_launches: Arc<Mutex<Vec<String>>>,
}

impl ProcessManager {
    pub fn new() -> Self {
        Self::new_with_recent(Vec::new())
    }

    pub fn new_with_recent(recent: Vec<String>) -> Self {
        Self {
            active_processes: Arc::new(Mutex::new(HashMap::new())),
            system: Arc::new(Mutex::new(System::new_all())),
            recent_launches: Arc::new(Mutex::new(recent)),
        }
    }

    /// Launch a browser profile with the given Chrome executable path.
    /// Returns `(pid, cdp_port)` so callers can connect via CDP.
    pub async fn launch_profile(
        &self,
        profile_id: &str,
        config: &AppConfig,
        chrome_path: &Path,
    ) -> Result<(u32, u16)> {
        let profile = config
            .profiles
            .iter()
            .find(|p| p.id == profile_id)
            .ok_or_else(|| BrowsionError::ProfileNotFound(profile_id.to_string()))?;

        if self.is_running(profile_id) {
            return Err(BrowsionError::Process(format!(
                "Profile {} is already running",
                profile_id
            )));
        }

        crate::config::validation::validate_chrome_path(chrome_path)?;

        let cdp_port = crate::process::port::allocate_cdp_port();
        let mut cmd = launcher::build_command(chrome_path, profile, cdp_port);

        tracing::info!(
            "Launching profile {} with CDP port {} — command: {:?}",
            profile_id,
            cdp_port,
            cmd
        );

        let child = cmd
            .spawn()
            .map_err(|e| BrowsionError::Process(format!("Failed to launch Chrome: {}", e)))?;

        let pid = child.id();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let process_info = ProcessInfo {
            profile_id: profile_id.to_string(),
            pid,
            launched_at: now,
            cdp_port: Some(cdp_port),
        };

        self.active_processes
            .lock()
            .insert(profile_id.to_string(), process_info);

        {
            let mut recent = self.recent_launches.lock();
            recent.retain(|id| id != profile_id);
            recent.insert(0, profile_id.to_string());
            if recent.len() > 10 {
                recent.truncate(10);
            }
        }

        tracing::info!(
            "Launched profile {} with PID {} on CDP port {}",
            profile_id,
            pid,
            cdp_port
        );

        Ok((pid, cdp_port))
    }

    /// Get the CDP port for a running profile (if available).
    pub fn get_cdp_port(&self, profile_id: &str) -> Option<u16> {
        let processes = self.active_processes.lock();
        processes
            .get(profile_id)
            .and_then(|info| info.cdp_port)
    }

    /// Kill a running browser profile
    pub async fn kill_profile(&self, profile_id: &str) -> Result<()> {
        let process_info = {
            let processes = self.active_processes.lock();
            processes.get(profile_id).cloned()
        };

        if let Some(info) = process_info {
            tracing::info!("Killing profile {} (PID: {})", profile_id, info.pid);

            // Try to kill the process
            let pid = Pid::from_u32(info.pid);
            let mut system = self.system.lock();
            system.refresh_processes_specifics(
                sysinfo::ProcessesToUpdate::Some(&[pid]),
                ProcessRefreshKind::new(),
            );

            if let Some(process) = system.process(pid) {
                if process.kill() {
                    tracing::info!("Successfully killed process {}", info.pid);
                } else {
                    tracing::warn!("Failed to kill process {}", info.pid);
                }
            } else {
                tracing::warn!("Process {} not found in system", info.pid);
            }

            // Remove from active processes
            self.active_processes.lock().remove(profile_id);

            Ok(())
        } else {
            Err(BrowsionError::Process(format!(
                "Profile {} is not running",
                profile_id
            )))
        }
    }

    /// Check if a profile is currently running
    pub fn is_running(&self, profile_id: &str) -> bool {
        let processes = self.active_processes.lock();
        if let Some(info) = processes.get(profile_id) {
            // Verify the process actually exists and is a Chrome process
            let pid = Pid::from_u32(info.pid);
            let mut system = self.system.lock();
            system.refresh_processes_specifics(
                sysinfo::ProcessesToUpdate::Some(&[pid]),
                ProcessRefreshKind::new(),
            );

            if let Some(process) = system.process(pid) {
                // Check if it's actually a Chrome/Chromium process and not a zombie
                let name = process.name().to_string_lossy().to_lowercase();
                let is_chrome = name.contains("chrome") || name.contains("chromium");
                let is_zombie = process.status() == sysinfo::ProcessStatus::Zombie;

                is_chrome && !is_zombie
            } else {
                false
            }
        } else {
            false
        }
    }

    /// Get process info for a profile
    pub fn get_process_info(&self, profile_id: &str) -> Option<ProcessInfo> {
        let processes = self.active_processes.lock();
        processes.get(profile_id).cloned()
    }

    /// Clean up dead processes from tracking.
    /// Returns the profile IDs that were removed so callers can clean up
    /// associated resources (e.g. CDP sessions).
    pub async fn cleanup_dead_processes(&self) -> Result<Vec<String>> {
        let mut to_remove = Vec::new();

        {
            let processes = self.active_processes.lock();
            let mut system = self.system.lock();

            for (profile_id, info) in processes.iter() {
                let pid = Pid::from_u32(info.pid);
                system.refresh_processes_specifics(
                    sysinfo::ProcessesToUpdate::Some(&[pid]),
                    ProcessRefreshKind::new(),
                );

                let should_remove = if let Some(process) = system.process(pid) {
                    let name = process.name().to_string_lossy().to_lowercase();
                    let is_chrome = name.contains("chrome") || name.contains("chromium");
                    let is_zombie = process.status() == sysinfo::ProcessStatus::Zombie;

                    if !is_chrome {
                        tracing::info!(
                            "Process {} for profile {} is no longer Chrome (name: {}), removing",
                            info.pid,
                            profile_id,
                            name
                        );
                        true
                    } else if is_zombie {
                        tracing::info!(
                            "Process {} for profile {} is a zombie, removing",
                            info.pid,
                            profile_id
                        );
                        true
                    } else {
                        false
                    }
                } else {
                    tracing::info!(
                        "Process {} for profile {} is dead, removing",
                        info.pid,
                        profile_id
                    );
                    true
                };

                if should_remove {
                    to_remove.push(profile_id.clone());
                }
            }
        }

        if !to_remove.is_empty() {
            let mut processes = self.active_processes.lock();
            for profile_id in &to_remove {
                processes.remove(profile_id);
            }
        }

        Ok(to_remove)
    }

    /// Get all running profile IDs
    pub fn get_running_profiles(&self) -> Vec<String> {
        let processes = self.active_processes.lock();
        processes.keys().cloned().collect()
    }

    /// Get recently launched profile IDs (most recent first)
    pub fn get_recent_launches(&self) -> Vec<String> {
        let recent = self.recent_launches.lock();
        recent.clone()
    }

    /// Register an externally-launched browser (e.g., one that survived a Tauri restart).
    /// Does NOT spawn a new process — just tracks the existing PID + CDP port.
    pub fn register_external(&self, profile_id: &str, pid: u32, cdp_port: u16) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let process_info = ProcessInfo {
            profile_id: profile_id.to_string(),
            pid,
            launched_at: now,
            cdp_port: Some(cdp_port),
        };

        self.active_processes
            .lock()
            .insert(profile_id.to_string(), process_info);

        tracing::info!(
            "Registered external session: profile={} pid={} cdp_port={}",
            profile_id,
            pid,
            cdp_port
        );
    }
}

impl Default for ProcessManager {
    fn default() -> Self {
        Self::new()
    }
}
