use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::config;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthState {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub token_type: String, // "oauth" or "pat"
    pub expires_at: Option<i64>,
    pub user_email: Option<String>,
}

impl AuthState {
    fn auth_file() -> PathBuf {
        config::get_config_dir().join("auth.json")
    }

    pub fn load() -> Result<Option<Self>> {
        let path = Self::auth_file();
        if !path.exists() {
            return Ok(None);
        }
        let content = std::fs::read_to_string(&path)?;
        let state: Self = serde_json::from_str(&content)?;
        Ok(Some(state))
    }

    pub fn save(&self) -> Result<()> {
        config::ensure_config_dir()?;
        let path = Self::auth_file();
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, content)?;
        Ok(())
    }

    pub fn clear() -> Result<()> {
        let path = Self::auth_file();
        if path.exists() {
            std::fs::remove_file(&path)?;
        }
        Ok(())
    }

    pub fn is_expired(&self) -> bool {
        if let Some(expires_at) = self.expires_at {
            let now = chrono::Utc::now().timestamp();
            now >= expires_at
        } else {
            false // PATs don't have expiration tracked here
        }
    }
}
