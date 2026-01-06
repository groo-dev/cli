use anyhow::{anyhow, Result};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use reqwest::Client;

use super::crypto::{decrypt, derive_key, encrypt};
use super::types::{KeyInfoResponse, Vault, VaultResponse};

const PASS_API_URL: &str = "https://pass.groo.dev/v1";

pub struct PassClient {
    token: String,
    client: Client,
}

impl PassClient {
    pub fn new(token: String) -> Self {
        Self {
            token,
            client: Client::new(),
        }
    }

    /// Get key derivation parameters (salt, iterations)
    pub async fn get_key_info(&self) -> Result<KeyInfoResponse> {
        let resp = self
            .client
            .get(format!("{}/vault/key-info", PASS_API_URL))
            .header("Cookie", format!("session={}", self.token))
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            if status.as_u16() == 404 {
                return Err(anyhow!(
                    "Vault not set up. Please create your vault at https://pass.groo.dev first."
                ));
            }
            if status.as_u16() == 401 {
                return Err(anyhow!(
                    "Not authenticated. Run 'groo auth login' first."
                ));
            }
            let text = resp.text().await.unwrap_or_default();
            return Err(anyhow!("Failed to get key info ({}): {}", status, text));
        }

        Ok(resp.json().await?)
    }

    /// Fetch encrypted vault
    pub async fn get_vault(&self) -> Result<VaultResponse> {
        let resp = self
            .client
            .get(format!("{}/vault", PASS_API_URL))
            .header("Cookie", format!("session={}", self.token))
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            if status.as_u16() == 404 {
                return Err(anyhow!(
                    "Vault not found. Please create your vault at https://pass.groo.dev first."
                ));
            }
            if status.as_u16() == 401 {
                return Err(anyhow!(
                    "Not authenticated. Run 'groo auth login' first."
                ));
            }
            let text = resp.text().await.unwrap_or_default();
            return Err(anyhow!("Failed to fetch vault ({}): {}", status, text));
        }

        Ok(resp.json().await?)
    }

    /// Fetch and decrypt vault with password verification
    /// Returns (decrypted vault, derived key, vault version)
    pub async fn unlock(&self, password: &str) -> Result<(Vault, [u8; 32], u32)> {
        // Get key derivation params
        let key_info = self.get_key_info().await?;
        let salt = BASE64
            .decode(&key_info.key_salt)
            .map_err(|e| anyhow!("Invalid key salt: {}", e))?;

        // Derive key
        let key = derive_key(password, &salt, key_info.kdf_iterations);

        // Fetch encrypted vault
        let vault_resp = self.get_vault().await?;

        // Decrypt
        let vault_json = decrypt(&vault_resp.encrypted_data, &vault_resp.iv, &key)?;
        let vault: Vault =
            serde_json::from_str(&vault_json).map_err(|e| anyhow!("Invalid vault data: {}", e))?;

        Ok((vault, key, vault_resp.version))
    }

    /// Update vault (with optimistic locking)
    pub async fn update_vault(
        &self,
        vault: &Vault,
        key: &[u8; 32],
        expected_version: u32,
    ) -> Result<VaultResponse> {
        let vault_json = serde_json::to_string(vault)?;
        let (encrypted_data, iv) = encrypt(&vault_json, key)?;

        let resp = self
            .client
            .put(format!("{}/vault", PASS_API_URL))
            .header("Cookie", format!("session={}", self.token))
            .json(&serde_json::json!({
                "encryptedData": encrypted_data,
                "iv": iv,
                "expectedVersion": expected_version,
            }))
            .send()
            .await?;

        if resp.status().as_u16() == 409 {
            return Err(anyhow!(
                "Vault was modified elsewhere. Please refresh and try again."
            ));
        }

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(anyhow!("Failed to update vault ({}): {}", status, text));
        }

        Ok(resp.json().await?)
    }
}
