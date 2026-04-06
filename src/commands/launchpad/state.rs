use anyhow::Result;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize, Deserialize)]
pub struct LaunchpadState {
    pub config_hash: String,
    pub completed_steps: Vec<CompletedStep>,
    pub created_resources: Vec<CreatedResource>,
    #[serde(skip)]
    path: PathBuf,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CompletedStep {
    pub step: String,
    pub project: Option<String>,
    pub result: StepResult,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum StepResult {
    Ok,
    Failed,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CreatedResource {
    #[serde(rename = "type")]
    pub resource_type: String,
    pub name: String,
    pub id: String,
}

impl LaunchpadState {
    pub fn new(config_hash: String, root: &Path) -> Self {
        Self {
            config_hash,
            completed_steps: Vec::new(),
            created_resources: Vec::new(),
            path: root.join(".launchpad-state.json"),
        }
    }

    pub fn load(root: &Path) -> Result<Option<Self>> {
        let path = root.join(".launchpad-state.json");
        if !path.exists() {
            return Ok(None);
        }
        let content = std::fs::read_to_string(&path)?;
        let mut state: Self = serde_json::from_str(&content)?;
        state.path = path;
        Ok(Some(state))
    }

    pub fn save(&self) -> Result<()> {
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(&self.path, content)?;
        Ok(())
    }

    pub fn delete(&self) -> Result<()> {
        if self.path.exists() {
            std::fs::remove_file(&self.path)?;
        }
        Ok(())
    }

    pub fn is_step_complete(&self, step: &str, project: Option<&str>) -> bool {
        self.completed_steps.iter().any(|s| {
            s.step == step
                && s.project.as_deref() == project
                && s.result == StepResult::Ok
        })
    }

    pub fn mark_complete(&mut self, step: &str, project: Option<&str>) {
        self.completed_steps.push(CompletedStep {
            step: step.to_string(),
            project: project.map(|s| s.to_string()),
            result: StepResult::Ok,
            error: None,
        });
    }

    #[allow(dead_code)]
    pub fn mark_failed(&mut self, step: &str, project: Option<&str>, error: &str) {
        self.completed_steps.push(CompletedStep {
            step: step.to_string(),
            project: project.map(|s| s.to_string()),
            result: StepResult::Failed,
            error: Some(error.to_string()),
        });
    }

    pub fn add_resource(&mut self, resource_type: &str, name: &str, id: &str) {
        self.created_resources.push(CreatedResource {
            resource_type: resource_type.to_string(),
            name: name.to_string(),
            id: id.to_string(),
        });
    }

    pub fn config_changed(&self, new_hash: &str) -> bool {
        self.config_hash != new_hash
    }

    pub fn first_failure_index(&self) -> Option<usize> {
        self.completed_steps
            .iter()
            .position(|s| s.result == StepResult::Failed)
    }
}

pub fn hash_config(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    format!("{:x}", hasher.finalize())
}
