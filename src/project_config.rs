use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

const GROO_DIR: &str = ".groo";
const CONFIG_FILE: &str = "config.json";

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ProjectConfig {
    #[serde(default)]
    pub selected_services: Vec<String>,
    #[serde(default)]
    pub selected_build_services: Vec<String>,
    #[serde(default)]
    pub selected_lint_services: Vec<String>,
}

impl ProjectConfig {
    pub fn load(project_root: &Path) -> Result<Self> {
        let config_file = get_config_file(project_root);
        if !config_file.exists() {
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(&config_file)?;
        let config: ProjectConfig = serde_json::from_str(&content)?;
        Ok(config)
    }

    pub fn save(&self, project_root: &Path) -> Result<()> {
        let groo_dir = project_root.join(GROO_DIR);
        if !groo_dir.exists() {
            std::fs::create_dir_all(&groo_dir)?;
        }
        let config_file = get_config_file(project_root);
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(&config_file, content)?;
        Ok(())
    }
}

pub fn get_groo_dir(project_root: &Path) -> PathBuf {
    project_root.join(GROO_DIR)
}

pub fn get_config_file(project_root: &Path) -> PathBuf {
    get_groo_dir(project_root).join(CONFIG_FILE)
}
