//! Pass-backed secret storage for CLI secrets.
//!
//! Stores CLI secrets (auth tokens, ops private keys) as encrypted notes in the pass vault.
//! This replaces keychain storage with a unified, cross-device solution.

use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;
use uuid::Uuid;

use super::client::PassClient;
use super::types::{NoteItem, Vault, VaultItem};

const CLI_OPS_PREFIX: &str = "groo-cli:ops:";

/// Pass-backed storage for CLI secrets
pub struct PassStorage {
    client: PassClient,
    key: [u8; 32],
    vault: Vault,
    version: u32,
}

impl PassStorage {
    /// Unlock pass vault with master password
    pub async fn unlock(token: &str, master_password: &str) -> Result<Self> {
        let client = PassClient::new(token.to_string());
        let (vault, key, version) = client.unlock(master_password).await?;
        Ok(Self {
            client,
            key,
            vault,
            version,
        })
    }

    /// Get ops private key from pass vault
    pub fn get_ops_key(&self, app_id: &str) -> Option<String> {
        self.get_note(&format!("{}{}", CLI_OPS_PREFIX, app_id))
    }

    /// Store ops private key in pass vault
    pub async fn set_ops_key(&mut self, app_id: &str, key: &str) -> Result<()> {
        self.set_note(&format!("{}{}", CLI_OPS_PREFIX, app_id), key)
            .await
    }

    /// Delete ops private key from pass vault
    pub async fn delete_ops_key(&mut self, app_id: &str) -> Result<()> {
        self.delete_note(&format!("{}{}", CLI_OPS_PREFIX, app_id))
            .await
    }

    /// Check if ops private key exists in pass vault
    pub fn has_ops_key(&self, app_id: &str) -> bool {
        self.get_ops_key(app_id).is_some()
    }

    // =========================================================================
    // Internal helpers
    // =========================================================================

    /// Find note by name (non-deleted only)
    fn get_note(&self, name: &str) -> Option<String> {
        self.vault.items.iter().find_map(|item| match item {
            VaultItem::Note(n) if n.name == name && n.deleted_at.is_none() => {
                Some(n.content.clone())
            }
            _ => None,
        })
    }

    /// Create or update note, then sync to server
    async fn set_note(&mut self, name: &str, content: &str) -> Result<()> {
        let now = now_timestamp();

        // Find existing note
        let existing_idx = self.vault.items.iter().position(|item| {
            matches!(item, VaultItem::Note(n) if n.name == name && n.deleted_at.is_none())
        });

        if let Some(idx) = existing_idx {
            // Update existing
            if let VaultItem::Note(ref mut note) = self.vault.items[idx] {
                note.content = content.to_string();
                note.updated_at = now;
            }
        } else {
            // Create new
            let note = NoteItem {
                id: Uuid::new_v4().to_string(),
                name: name.to_string(),
                content: content.to_string(),
                folder_id: None,
                favorite: None,
                created_at: now,
                updated_at: now,
                deleted_at: None,
            };
            self.vault.items.push(VaultItem::Note(note));
        }

        self.sync().await
    }

    /// Soft-delete note (set deleted_at), then sync to server
    async fn delete_note(&mut self, name: &str) -> Result<()> {
        let now = now_timestamp();

        // Find and soft-delete
        for item in &mut self.vault.items {
            if let VaultItem::Note(note) = item {
                if note.name == name && note.deleted_at.is_none() {
                    note.deleted_at = Some(now);
                    note.updated_at = now;
                    break;
                }
            }
        }

        self.sync().await
    }

    /// Sync vault to server
    async fn sync(&mut self) -> Result<()> {
        self.vault.last_modified = now_timestamp();
        let resp = self
            .client
            .update_vault(&self.vault, &self.key, self.version)
            .await?;
        self.version = resp.version;
        Ok(())
    }
}

fn now_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
}
