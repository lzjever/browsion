use crate::config::schema::{AppConfig, BrowserProfile, ProcessInfo};
use crate::error::{BrowsionError, Result};
use crate::process::launcher;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use sysinfo::{System, Pid, ProcessRefreshKind};

pub struct ProcessManager {
    /// Map of profile_id -> ProcessInfo
    active_processes: Arc<Mutex<HashMap<String, ProcessInfo>>>,
    /// System info for process tracking
    system: Arc<Mutex<System>>,
}

impl ProcessManager {
    pub fn new() -> Self {
        Self {
            active_processes: Arc::new(Mutex::new(HashMap::new())),
            system: Arc::new(Mutex::new(System::new_all())),
        }
    }

    /// Launch a browser profile
    pub async fn launch_profile(&self, profile_id: &str, config: &AppConfig) -> Result<u32> {
        // Find the profile
        let profile = config
            .profiles
            .iter()
            .find(|p| p.id == profile_id)
            .ok_or_else(|| BrowsionError::ProfileNotFound(profile_id.to_string()))?;

        // Check if already running
        if self.is_running(profile_id) {
            return Err(BrowsionError::Process(format!(
                "Profile {} is already running",
                profile_id
            )));
        }

        // Validate Chrome path
        crate::config::validation::validate_chrome_path(&config.chrome_path)?;

        // Build and execute command
        let mut cmd = launcher::build_command(&config.chrome_path, profile);

        tracing::info!(
            "Launching profile {} with command: {:?}",
            profile_id,
            cmd
        );

        let child = cmd.spawn().map_err(|e| {
            BrowsionError::Process(format!("Failed to launch Chrome: {}", e))
        })?;

        let pid = child.id();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let process_info = ProcessInfo {
            profile_id: profile_id.to_string(),
            pid,
            launched_at: now,
        };

        // Store process info
        self.active_processes
            .lock()
            .insert(profile_id.to_string(), process_info);

        tracing::info!("Launched profile {} with PID {}", profile_id, pid);

        Ok(pid)
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
            // Verify the process actually exists
            let pid = Pid::from_u32(info.pid);
            let mut system = self.system.lock();
            system.refresh_processes_specifics(
                sysinfo::ProcessesToUpdate::Some(&[pid]),
                ProcessRefreshKind::new(),
            );
            system.process(pid).is_some()
        } else {
            false
        }
    }

    /// Get process info for a profile
    pub fn get_process_info(&self, profile_id: &str) -> Option<ProcessInfo> {
        let processes = self.active_processes.lock();
        processes.get(profile_id).cloned()
    }

    /// Clean up dead processes from tracking
    pub async fn cleanup_dead_processes(&self) -> Result<()> {
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

                if system.process(pid).is_none() {
                    tracing::info!(
                        "Process {} for profile {} is dead, removing",
                        info.pid,
                        profile_id
                    );
                    to_remove.push(profile_id.clone());
                }
            }
        }

        if !to_remove.is_empty() {
            let mut processes = self.active_processes.lock();
            for profile_id in to_remove {
                processes.remove(&profile_id);
            }
        }

        Ok(())
    }

    /// Get all running profile IDs
    pub fn get_running_profiles(&self) -> Vec<String> {
        let processes = self.active_processes.lock();
        processes.keys().cloned().collect()
    }
}

impl Default for ProcessManager {
    fn default() -> Self {
        Self::new()
    }
}
