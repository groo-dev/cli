use anyhow::{anyhow, Result};
use keyring::Entry;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::config;

const SERVICE_NAME: &str = "groo-cli";
const ACCOUNT_NAME: &str = "auth";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthState {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub token_type: String, // "oauth" or "pat"
    pub expires_at: Option<i64>,
    pub user_email: Option<String>,
}

impl AuthState {
    fn legacy_auth_file() -> PathBuf {
        config::get_config_dir().join("auth.json")
    }

    /// Load auth state from keychain, with migration from legacy file
    pub fn load() -> Result<Option<Self>> {
        // Try keychain first
        if let Some(state) = Self::load_from_keychain()? {
            return Ok(Some(state));
        }

        // Check for legacy JSON file and migrate
        if let Some(state) = Self::load_from_legacy_file()? {
            // Try to migrate to keychain
            if state.save().is_ok() {
                // Successfully migrated, delete legacy file
                Self::delete_legacy_file();
            }
            return Ok(Some(state));
        }

        Ok(None)
    }

    fn load_from_keychain() -> Result<Option<Self>> {
        let entry = match Entry::new(SERVICE_NAME, ACCOUNT_NAME) {
            Ok(e) => e,
            Err(_) => return Ok(None), // Keychain not available
        };

        match entry.get_password() {
            Ok(json) => {
                let state: Self = serde_json::from_str(&json)?;
                Ok(Some(state))
            }
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(keyring::Error::NoStorageAccess(_)) => Ok(None), // No keychain available
            Err(e) => Err(anyhow!("Keychain error: {}", e)),
        }
    }

    fn load_from_legacy_file() -> Result<Option<Self>> {
        let path = Self::legacy_auth_file();
        if !path.exists() {
            return Ok(None);
        }
        let content = std::fs::read_to_string(&path)?;
        let state: Self = serde_json::from_str(&content)?;
        Ok(Some(state))
    }

    fn delete_legacy_file() {
        let path = Self::legacy_auth_file();
        if path.exists() {
            let _ = std::fs::remove_file(&path);
        }
    }

    /// Save auth state to keychain (with Linux fallback)
    pub fn save(&self) -> Result<()> {
        // Try keychain first
        if let Ok(entry) = Entry::new(SERVICE_NAME, ACCOUNT_NAME) {
            let json = serde_json::to_string(self)?;
            if entry.set_password(&json).is_ok() {
                return Ok(());
            }
        }

        // Keychain unavailable - handle platform-specific fallback
        self.save_with_fallback()
    }

    #[cfg(target_os = "linux")]
    fn save_with_fallback(&self) -> Result<()> {
        // Prompt user for confirmation on Linux
        if !Self::confirm_file_fallback()? {
            return Err(anyhow!(
                "Token not saved. You'll need to login again next session."
            ));
        }
        self.save_to_file_with_permissions()
    }

    #[cfg(not(target_os = "linux"))]
    fn save_with_fallback(&self) -> Result<()> {
        Err(anyhow!("Failed to access system keychain"))
    }

    #[cfg(target_os = "linux")]
    fn confirm_file_fallback() -> Result<bool> {
        use console::style;
        use dialoguer::Confirm;

        println!(
            "{}",
            style("⚠ Secure keyring not available (no Secret Service)").yellow()
        );
        Confirm::new()
            .with_prompt("Save token to file with restricted permissions (600)?")
            .default(false)
            .interact()
            .map_err(|e| anyhow!("Prompt failed: {}", e))
    }

    #[cfg(target_os = "linux")]
    fn save_to_file_with_permissions(&self) -> Result<()> {
        use std::os::unix::fs::PermissionsExt;

        config::ensure_config_dir()?;
        let path = Self::legacy_auth_file();
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, &json)?;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
        Ok(())
    }

    /// Clear auth state from keychain (and legacy file)
    pub fn clear() -> Result<()> {
        // Clear from keychain
        if let Ok(entry) = Entry::new(SERVICE_NAME, ACCOUNT_NAME) {
            match entry.delete_credential() {
                Ok(()) | Err(keyring::Error::NoEntry) => {}
                Err(e) => {
                    // Log but don't fail - might not have keychain access
                    eprintln!("Warning: Could not clear keychain: {}", e);
                }
            }
        }

        // Also clear legacy file if it exists
        Self::delete_legacy_file();

        Ok(())
    }

}
