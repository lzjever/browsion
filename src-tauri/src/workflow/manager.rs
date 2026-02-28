//! Workflow manager: persists and retrieves workflow definitions.

use crate::workflow::schema::Workflow;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use std::path::PathBuf;

/// Directory where workflow definitions are stored.
fn workflows_dir() -> Result<PathBuf, std::io::Error> {
    let base = dirs::home_dir()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "No home directory"))?;
    let browsion_dir = base.join(".browsion");
    std::fs::create_dir_all(&browsion_dir)?;
    let workflows_dir = browsion_dir.join("workflows");
    std::fs::create_dir_all(&workflows_dir)?;
    Ok(workflows_dir)
}

/// Workflow file path.
fn workflow_path(id: &str) -> Result<PathBuf, std::io::Error> {
    Ok(workflows_dir()?.join(format!("{}.json", id)))
}

/// Manages workflow definitions.
pub struct WorkflowManager {
    workflows: Arc<RwLock<HashMap<String, Workflow>>>,
}

impl WorkflowManager {
    pub fn new() -> Result<Self, std::io::Error> {
        let manager = Self {
            workflows: Arc::new(RwLock::new(HashMap::new())),
        };
        manager.load_all()?;
        Ok(manager)
    }

    /// Load all workflows from disk.
    fn load_all(&self) -> Result<(), std::io::Error> {
        let dir = workflows_dir()?;
        let mut map = self.workflows.write();

        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                match self.load_one(&path) {
                    Ok(workflow) => {
                        map.insert(workflow.id.clone(), workflow);
                    }
                    Err(e) => {
                        tracing::warn!("Failed to load workflow from {:?}: {}", path, e);
                    }
                }
            }
        }

        Ok(())
    }

    /// Load a single workflow file.
    fn load_one(&self, path: &PathBuf) -> Result<Workflow, std::io::Error> {
        let content = std::fs::read_to_string(path)?;
        let workflow: Workflow = serde_json::from_str(&content)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        Ok(workflow)
    }

    /// List all workflows.
    pub fn list(&self) -> Vec<Workflow> {
        self.workflows.read().values().cloned().collect()
    }

    /// Get a workflow by ID.
    pub fn get(&self, id: &str) -> Option<Workflow> {
        self.workflows.read().get(id).cloned()
    }

    /// Create or update a workflow.
    pub fn save(&self, workflow: Workflow) -> Result<(), std::io::Error> {
        let path = workflow_path(&workflow.id)?;

        // Update timestamp
        let mut workflow = workflow;
        workflow.updated_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        // Serialize and write atomically
        let content = serde_json::to_string_pretty(&workflow)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

        let tmp_path = path.with_extension("json.tmp");
        std::fs::write(&tmp_path, content)?;
        std::fs::rename(&tmp_path, &path)?;

        // Update in-memory cache
        self.workflows.write().insert(workflow.id.clone(), workflow);

        Ok(())
    }

    /// Delete a workflow.
    pub fn delete(&self, id: &str) -> Result<(), std::io::Error> {
        let path = workflow_path(id)?;

        if path.exists() {
            std::fs::remove_file(path)?;
        }

        self.workflows.write().remove(id);

        Ok(())
    }
}

impl Default for WorkflowManager {
    fn default() -> Self {
        Self::new().unwrap_or_else(|e| {
            tracing::error!("Failed to create WorkflowManager: {}", e);
            Self {
                workflows: Arc::new(RwLock::new(HashMap::new())),
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workflow::schema::{StepType, WorkflowStep};

    #[test]
    fn test_workflow_crud() {
        let manager = WorkflowManager::new().unwrap();

        let workflow = Workflow {
            id: "test-crud".to_string(),
            name: "Test CRUD".to_string(),
            description: "Testing CRUD operations".to_string(),
            steps: vec![],
            variables: HashMap::new(),
            created_at: 0,
            updated_at: 0,
        };

        // Create
        manager.save(workflow.clone()).unwrap();

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
