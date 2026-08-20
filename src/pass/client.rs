use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use anyhow::{Result, anyhow};
use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use reqwest::{Client, Method, Response};
use serde::Serialize;

use super::crypto::{decrypt_record, derive_key, encrypt_record, unwrap_key};
use super::types::{
    Folder, KeyInfoResponse, RecordWriteRequest, RecordWriteResponse, RecordsResponse, Vault,
    VaultItem,
};
use crate::auth::provider;

const PASS_API_URL: &str = "https://pass.groo.dev/v1";

#[derive(Clone)]
pub struct PassClient {
    client: Client,
    versions: Arc<Mutex<HashMap<String, u32>>>,
}

impl PassClient {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
            versions: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    async fn send<B: Serialize + ?Sized>(
        &self,
        method: Method,
        path: &str,
        body: Option<&B>,
    ) -> Result<Response> {
        let token = provider::get_valid_auth().await?.access_token;
        let mut request = self
            .client
            .request(method, format!("{PASS_API_URL}{path}"))
            .bearer_auth(token);
        if let Some(value) = body {
            request = request.json(value);
        }
        let response = request.send().await?;
        if response.status().as_u16() == 401 {
            return Err(anyhow!(
                "Pass rejected the current OAuth session. Run 'groo auth login' again."
            ));
        }
        Ok(response)
    }

    async fn get_json<T: serde::de::DeserializeOwned>(&self, path: &str) -> Result<T> {
        let response = self.send::<()>(Method::GET, path, None).await?;
        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(anyhow!("Pass request failed ({}): {}", status, text));
        }
        Ok(response.json().await?)
    }

    pub async fn get_key_info(&self) -> Result<KeyInfoResponse> {
        self.get_json("/vault/key-info")
            .await
            .map_err(|error| anyhow!("Unable to read Pass vault key information: {error}"))
    }

    pub async fn unlock(&self, password: &str) -> Result<(Vault, [u8; 32], u32)> {
        let key_info = self.get_key_info().await?;
        if key_info.format_version != 2 {
            return Err(anyhow!(
                "This vault still uses legacy format {}. Open https://pass.groo.dev to convert it before using the CLI.",
                key_info.format_version
            ));
        }
        let salt = BASE64
            .decode(&key_info.key_salt)
            .map_err(|e| anyhow!("Invalid key salt: {e}"))?;
        let wrapping_key = derive_key(password, &salt, key_info.kdf_iterations);
        let vault_key = unwrap_key(
            &key_info.wrapped_vault_key,
            &key_info.wrap_iv,
            &wrapping_key,
        )
        .map_err(|_| anyhow!("Unable to unlock vault. Check your master password."))?;

        let mut cursor = 0;
        let mut items = Vec::new();
        let mut folders = Vec::new();
        let mut versions = HashMap::new();
        loop {
            let page: RecordsResponse = self
                .get_json(&format!("/vault/records?since={cursor}"))
                .await?;
            if page.format_version != 2 {
                return Err(anyhow!("Pass changed vault format during sync"));
            }
            let before = cursor;
            for record in page.records {
                cursor = cursor.max(record.seq);
                if record.is_deleted {
                    continue;
                }
                let (kind, data) = decrypt_record(&record, &vault_key)
                    .map_err(|e| anyhow!("Unable to decrypt Pass record {}: {e}", record.id))?;
                versions.insert(record.id.clone(), record.version);
                match kind.as_str() {
                    "item" => items.push(
                        serde_json::from_value::<VaultItem>(data)
                            .map_err(|e| anyhow!("Invalid item record {}: {e}", record.id))?,
                    ),
                    "folder" => folders.push(
                        serde_json::from_value::<Folder>(data)
                            .map_err(|e| anyhow!("Invalid folder record {}: {e}", record.id))?,
                    ),
                    _ => unreachable!(),
                }
            }
            cursor = cursor.max(page.next_seq);
            if !page.has_more {
                break;
            }
            if cursor <= before {
                return Err(anyhow!("Pass record pagination did not advance"));
            }
        }
        *self
            .versions
            .lock()
            .map_err(|_| anyhow!("Pass record state lock poisoned"))? = versions;
        Ok((
            Vault {
                version: 2,
                items,
                folders,
                last_modified: 0,
                rsa_private_key: None,
            },
            vault_key,
            cursor,
        ))
    }

    pub async fn save_item(
        &self,
        item: &VaultItem,
        vault_key: &[u8; 32],
    ) -> Result<RecordWriteResponse> {
        self.save_record(item.id(), "item", serde_json::to_value(item)?, vault_key)
            .await
    }

    async fn save_record(
        &self,
        id: &str,
        kind: &str,
        data: serde_json::Value,
        vault_key: &[u8; 32],
    ) -> Result<RecordWriteResponse> {
        let current = self
            .versions
            .lock()
            .map_err(|_| anyhow!("Pass record state lock poisoned"))?
            .get(id)
            .copied();
        let mut request: RecordWriteRequest = encrypt_record(id, kind, &data, vault_key)?;
        request.expected_version = current;
        let (method, path) = if current.is_some() {
            (
                Method::PUT,
                format!("/vault/records/{}", urlencoding::encode(id)),
            )
        } else {
            (Method::POST, "/vault/records".to_owned())
        };
        let response = self.send(method, &path, Some(&request)).await?;
        if response.status().as_u16() == 409 {
            return Err(anyhow!(
                "Pass record {id} was modified elsewhere. Re-run the command; no data was overwritten."
            ));
        }
        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(anyhow!(
                "Failed to save Pass record {id} ({status}): {text}"
            ));
        }
        let saved: RecordWriteResponse = response.json().await?;
        self.versions
            .lock()
            .map_err(|_| anyhow!("Pass record state lock poisoned"))?
            .insert(id.to_owned(), saved.version);
        Ok(saved)
    }
}
