//! Recording manager: persists and retrieves recordings.

use crate::recording::schema::Recording;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::path::PathBuf;

/// Directory where recordings are stored.
fn recordings_dir() -> Result<PathBuf, std::io::Error> {
    let base = dirs::home_dir()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "No home directory"))?;
    let browsion_dir = base.join(".browsion");
    std::fs::create_dir_all(&browsion_dir)?;
    let recordings_dir = browsion_dir.join("recordings");
    std::fs::create_dir_all(&recordings_dir)?;
    Ok(recordings_dir)
}

/// Recording file path.
fn recording_path(id: &str) -> Result<PathBuf, std::io::Error> {
    Ok(recordings_dir()?.join(format!("{}.json", id)))
}

/// Manages recording persistence.
pub struct RecordingManager {
    recordings: RwLock<HashMap<String, Recording>>,
}

impl RecordingManager {
    pub fn new() -> Result<Self, std::io::Error> {
        let manager = Self {
            recordings: RwLock::new(HashMap::new()),
        };
        manager.load_all()?;
        Ok(manager)
    }

    /// Load all recordings from disk.
    fn load_all(&self) -> Result<(), std::io::Error> {
        let dir = recordings_dir()?;
        let mut map = self.recordings.write();

        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                match self.load_one(&path) {
                    Ok(recording) => {
                        map.insert(recording.id.clone(), recording);
                    }
                    Err(e) => {
                        tracing::warn!("Failed to load recording from {:?}: {}", path, e);
                    }
                }
            }
        }

        Ok(())
    }

    /// Load a single recording file.
    fn load_one(&self, path: &PathBuf) -> Result<Recording, std::io::Error> {
        let content = std::fs::read_to_string(path)?;
        let recording: Recording = serde_json::from_str(&content)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        Ok(recording)
    }

    /// List all recordings.
    pub fn list(&self) -> Vec<Recording> {
        self.recordings.read().values().cloned().collect()
    }

    /// Get a recording by ID.
    pub fn get(&self, id: &str) -> Option<Recording> {
        self.recordings.read().get(id).cloned()
    }

    /// Save a recording.
    pub fn save(&self, recording: Recording) -> Result<(), std::io::Error> {
        let path = recording_path(&recording.id)?;

        // Serialize and write atomically
        let content = serde_json::to_string_pretty(&recording)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

        let tmp_path = path.with_extension("json.tmp");
        std::fs::write(&tmp_path, content)?;
        std::fs::rename(&tmp_path, &path)?;

        // Update in-memory cache
        self.recordings.write().insert(recording.id.clone(), recording);

        Ok(())
    }

    /// Delete a recording.
    pub fn delete(&self, id: &str) -> Result<(), std::io::Error> {
        let path = recording_path(id)?;

        if path.exists() {
            std::fs::remove_file(path)?;
        }

        self.recordings.write().remove(id);

        Ok(())
    }
}

impl Default for RecordingManager {
    fn default() -> Self {
        Self::new().unwrap_or_else(|e| {
            tracing::error!("Failed to create RecordingManager: {}", e);
            Self {
                recordings: RwLock::new(HashMap::new()),
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recording::schema::RecordedActionType;

    #[test]
    fn test_recording_crud() {
        let manager = RecordingManager::new().unwrap();

        let recording = Recording {
            id: "test-crud".to_string(),
            name: "Test CRUD".to_string(),
            description: "Testing CRUD operations".to_string(),
            profile_id: "profile-1".to_string(),
            actions: vec![],
            created_at: 0,
            duration_ms: 1000,
        };

        // Create
        manager.save(recording.clone()).unwrap();

        // Read
        let retrieved = manager.get("test-crud").unwrap();
        assert_eq!(retrieved.id, "test-crud");
        assert_eq!(retrieved.name, "Test CRUD");

        // Update
        let mut updated = retrieved;
        updated.name = "Updated Name".to_string();
        manager.save(updated).unwrap();
        let retrieved = manager.get("test-crud").unwrap();
        assert_eq!(retrieved.name, "Updated Name");

        // Delete
        manager.delete("test-crud").unwrap();
        assert!(manager.get("test-crud").is_none());
    }
}
