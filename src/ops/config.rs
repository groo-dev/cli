use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

const OPS_CONFIG_FILE: &str = "ops.json";

/// Ops configuration stored in .groo/ops.json
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OpsConfig {
    #[serde(default)]
    pub services: HashMap<String, ServiceLink>,
}

/// Link between a local service and an ops application
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceLink {
    pub application_id: String,
    pub application_name: String,
    /// Public key in JWK format (JSON string)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub public_key: Option<String>,
}

impl OpsConfig {
    /// Load ops config from .groo/ops.json
    pub fn load(project_root: &Path) -> Result<Self> {
        let path = project_root.join(".groo").join(OPS_CONFIG_FILE);
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = fs::read_to_string(&path)?;
        Ok(serde_json::from_str(&content)?)
    }

    /// Save ops config to .groo/ops.json
    pub fn save(&self, project_root: &Path) -> Result<()> {
        let groo_dir = project_root.join(".groo");
        fs::create_dir_all(&groo_dir)?;
        let content = serde_json::to_string_pretty(self)?;
        fs::write(groo_dir.join(OPS_CONFIG_FILE), content)?;
        Ok(())
    }

    /// Get link for a service
    pub fn get_service(&self, name: &str) -> Option<&ServiceLink> {
        self.services.get(name)
    }

    /// Set link for a service
    pub fn set_service(&mut self, name: String, link: ServiceLink) {
        self.services.insert(name, link);
    }

    /// Remove link for a service
    pub fn remove_service(&mut self, name: &str) -> Option<ServiceLink> {
        self.services.remove(name)
    }

    /// Get all linked service names
    #[allow(dead_code)]
    pub fn service_names(&self) -> Vec<&String> {
        self.services.keys().collect()
    }
}
