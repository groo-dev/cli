//! Auth state storage using local encrypted file.
//!
//! Stores auth tokens in `~/.groo/auth.enc` encrypted with master password.
//! No keychain dependency - uses the same PBKDF2 + AES-GCM as pass vault.

use anyhow::{anyhow, Result};
use rpassword::prompt_password;
use serde::{Deserialize, Serialize};

use crate::config;
use crate::pass::crypto::{decrypt, derive_key, encrypt};

/// Prompt for master password and load auth state
pub fn load_auth_with_password() -> Result<(AuthState, String)> {
    if !AuthState::exists() {
        return Err(anyhow!("Not logged in. Run 'groo auth login' first."));
    }

    let master_password = prompt_password("🔑 Master password: ")?;
    let auth = AuthState::load(&master_password)?
        .ok_or_else(|| anyhow!("Failed to load auth state"))?;

    Ok((auth, master_password))
}

const AUTH_FILE: &str = "auth.enc";
const AUTH_SALT_FILE: &str = "auth.salt";
const KDF_ITERATIONS: u32 = 600_000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthState {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub token_type: String, // "oauth" or "pat"
    pub expires_at: Option<i64>,
    pub user_email: Option<String>,
}

/// Encrypted auth file format
#[derive(Debug, Serialize, Deserialize)]
struct EncryptedAuth {
    data: String, // base64 ciphertext
    iv: String,   // base64 IV
}

impl AuthState {
    /// Load auth state from encrypted file
    pub fn load(master_password: &str) -> Result<Option<Self>> {
        let auth_path = config::get_config_dir().join(AUTH_FILE);
        let salt_path = config::get_config_dir().join(AUTH_SALT_FILE);

        if !auth_path.exists() || !salt_path.exists() {
            return Ok(None);
        }

        // Read salt
        let salt = std::fs::read(&salt_path)?;

        // Derive key
        let key = derive_key(master_password, &salt, KDF_ITERATIONS);

        // Read encrypted auth
        let encrypted_json = std::fs::read_to_string(&auth_path)?;
        let encrypted: EncryptedAuth = serde_json::from_str(&encrypted_json)
            .map_err(|_| anyhow!("Invalid auth file format"))?;

        // Decrypt
        let auth_json = decrypt(&encrypted.data, &encrypted.iv, &key)
            .map_err(|_| anyhow!("Failed to decrypt auth. Wrong master password?"))?;

        let state: Self = serde_json::from_str(&auth_json)?;
        Ok(Some(state))
    }

    /// Check if auth file exists (without decrypting)
    pub fn exists() -> bool {
        let auth_path = config::get_config_dir().join(AUTH_FILE);
        let salt_path = config::get_config_dir().join(AUTH_SALT_FILE);
        auth_path.exists() && salt_path.exists()
    }

    /// Save auth state to encrypted file
    pub fn save(&self, master_password: &str) -> Result<()> {
        config::ensure_config_dir()?;

        let auth_path = config::get_config_dir().join(AUTH_FILE);
        let salt_path = config::get_config_dir().join(AUTH_SALT_FILE);

        // Generate or read salt
        let salt = if salt_path.exists() {
            std::fs::read(&salt_path)?
        } else {
            let mut salt = vec![0u8; 32];
            use rand::RngCore;
            rand::thread_rng().fill_bytes(&mut salt);
            std::fs::write(&salt_path, &salt)?;
            salt
        };

        // Derive key
        let key = derive_key(master_password, &salt, KDF_ITERATIONS);

        // Encrypt
        let auth_json = serde_json::to_string(self)?;
        let (data, iv) = encrypt(&auth_json, &key)?;

        // Save
        let encrypted = EncryptedAuth { data, iv };
        let encrypted_json = serde_json::to_string_pretty(&encrypted)?;
        std::fs::write(&auth_path, encrypted_json)?;

        Ok(())
    }

    /// Clear auth state (delete files)
    pub fn clear() -> Result<()> {
        let auth_path = config::get_config_dir().join(AUTH_FILE);
        let salt_path = config::get_config_dir().join(AUTH_SALT_FILE);

        if auth_path.exists() {
            std::fs::remove_file(&auth_path)?;
        }
        if salt_path.exists() {
            std::fs::remove_file(&salt_path)?;
        }

        Ok(())
    }
}
