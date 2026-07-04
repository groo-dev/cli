use anyhow::{anyhow, Result};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use futures::{SinkExt, StreamExt};
use rand::Rng;
use tokio_tungstenite::{connect_async, tungstenite::Message};

use super::crypto::{decrypt_file, derive_key, encrypt, encrypt_file, verify_key};
use super::types::{
    ClientMessage, FileAttachment, FileUploadResponse, ListItem, ServerMessage, UserState,
};

const PAD_WS_URL: &str = "wss://pad.groo.dev/v1/ws";
const PAD_API_URL: &str = "https://pad.groo.dev/v1";

pub struct PadClient {
    token: String,
}

impl PadClient {
    pub fn new(token: String) -> Self {
        Self { token }
    }

    pub async fn add_list_item(
        &self,
        text: Option<&str>,
        files: Vec<FileAttachment>,
        password: &str,
    ) -> Result<()> {
        // Connect to WebSocket
        let request = http::Request::builder()
            .uri(PAD_WS_URL)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Host", "pad.groo.dev")
            .header("Upgrade", "websocket")
            .header("Connection", "Upgrade")
            .header("Sec-WebSocket-Version", "13")
            .header("Sec-WebSocket-Key", generate_ws_key())
            .body(())?;

        let (mut ws, _) = connect_async(request)
            .await
            .map_err(|e| anyhow!("WebSocket connection failed: {}", e))?;

        // Wait for sync message
        let sync_msg = ws
            .next()
            .await
            .ok_or_else(|| anyhow!("Connection closed before sync"))??;

        let state: UserState = match sync_msg {
            Message::Text(text) => {
                let msg: ServerMessage = serde_json::from_str(&text)?;
                match msg {
                    ServerMessage::Sync { state } => state,
                    ServerMessage::Error { message } => {
                        return Err(anyhow!("Server error: {}", message))
                    }
                    _ => return Err(anyhow!("Expected sync message")),
                }
            }
            _ => return Err(anyhow!("Expected text message")),
        };

        // Get encryption salt
        let salt_b64 = state
            .encryption_salt
            .ok_or_else(|| anyhow!("Encryption not set up. Please set up encryption in the web app first."))?;
        let salt = BASE64.decode(&salt_b64)?;

        // Derive key
        let key = derive_key(password, &salt);

        // Verify password
        if let Some(ref test) = state.encryption_test
            && !verify_key(test, &key) {
                return Err(anyhow!("Incorrect encryption password"));
            }

        // Encrypt text
        let encrypted_text = if let Some(t) = text {
            encrypt(t, &key)?
        } else {
            encrypt("", &key)?
        };

        // Create list item
        let item = ListItem {
            id: uuid::Uuid::new_v4().to_string(),
            encrypted_text,
            files,
            created_at: chrono::Utc::now().timestamp_millis(),
        };

        // Send add message
        let msg = ClientMessage::ListAdd { item };
        ws.send(Message::Text(serde_json::to_string(&msg)?.into()))
            .await?;

        // Wait for confirmation
        if let Some(Ok(confirm)) = ws.next().await
            && let Message::Text(text) = confirm {
                let response: ServerMessage = serde_json::from_str(&text)?;
                match response {
                    ServerMessage::ListAdded { .. } => {}
                    ServerMessage::Error { message } => {
                        return Err(anyhow!("Server error: {}", message))
                    }
                    _ => {}
                }
            }

        // Close connection
        ws.close(None).await?;

        Ok(())
    }

    pub async fn upload_file(
        &self,
        data: &[u8],
        file_name: &str,
        mime_type: &str,
        key: &[u8; 32],
    ) -> Result<FileAttachment> {
        // Encrypt file content
        let encrypted_data = encrypt_file(data, key)?;

        // Upload via HTTP
        let client = reqwest::Client::new();
        let form = reqwest::multipart::Form::new().part(
            "file",
            reqwest::multipart::Part::bytes(encrypted_data)
                .file_name("encrypted")
                .mime_str("application/octet-stream")?,
        );

        let resp = client
            .post(format!("{}/files", PAD_API_URL))
            .bearer_auth(&self.token)
            .multipart(form)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(anyhow!("Upload failed ({}): {}", status, text));
        }

        let upload_resp: FileUploadResponse = resp.json().await?;

        // Encrypt metadata
        let encrypted_name = encrypt(file_name, key)?;
        let encrypted_type = encrypt(mime_type, key)?;

        Ok(FileAttachment {
            id: upload_resp.id,
            encrypted_name,
            encrypted_type,
            size: upload_resp.size,
            r2_key: upload_resp.r2_key,
        })
    }

    pub async fn get_encryption_salt(&self, password: &str) -> Result<[u8; 32]> {
        let (_, key) = self.connect_and_sync(password).await?;
        Ok(key)
    }

    /// Connect to WebSocket, get initial state, and derive encryption key
    pub async fn connect_and_sync(&self, password: &str) -> Result<(UserState, [u8; 32])> {
        let request = http::Request::builder()
            .uri(PAD_WS_URL)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Host", "pad.groo.dev")
            .header("Upgrade", "websocket")
            .header("Connection", "Upgrade")
            .header("Sec-WebSocket-Version", "13")
            .header("Sec-WebSocket-Key", generate_ws_key())
            .body(())?;

        let (mut ws, _) = connect_async(request).await?;

        let sync_msg = ws
            .next()
            .await
            .ok_or_else(|| anyhow!("Connection closed"))??;

        let state: UserState = match sync_msg {
            Message::Text(text) => {
                let msg: ServerMessage = serde_json::from_str(&text)?;
                match msg {
                    ServerMessage::Sync { state } => state,
                    _ => return Err(anyhow!("Expected sync message")),
                }
            }
            _ => return Err(anyhow!("Expected text message")),
        };

        ws.close(None).await?;

        let salt_b64 = state
            .encryption_salt
            .clone()
            .ok_or_else(|| anyhow!("Encryption not set up"))?;
        let salt = BASE64.decode(&salt_b64)?;
        let key = derive_key(password, &salt);

        if let Some(ref test) = state.encryption_test
            && !verify_key(test, &key) {
                return Err(anyhow!("Incorrect encryption password"));
            }

        Ok((state, key))
    }

    /// Fetch current state without password verification (for refresh when key is already known)
    pub async fn fetch_state(&self) -> Result<UserState> {
        let request = http::Request::builder()
            .uri(PAD_WS_URL)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Host", "pad.groo.dev")
            .header("Upgrade", "websocket")
            .header("Connection", "Upgrade")
            .header("Sec-WebSocket-Version", "13")
            .header("Sec-WebSocket-Key", generate_ws_key())
            .body(())?;

        let (mut ws, _) = connect_async(request).await?;

        let sync_msg = ws
            .next()
            .await
            .ok_or_else(|| anyhow!("Connection closed"))??;

        let state: UserState = match sync_msg {
            Message::Text(text) => {
                let msg: ServerMessage = serde_json::from_str(&text)?;
                match msg {
                    ServerMessage::Sync { state } => state,
                    _ => return Err(anyhow!("Expected sync message")),
                }
            }
            _ => return Err(anyhow!("Expected text message")),
        };

        ws.close(None).await?;
        Ok(state)
    }

    /// Download and decrypt a file from R2 storage
    pub async fn download_file(&self, r2_key: &str, key: &[u8; 32]) -> Result<Vec<u8>> {
        let client = reqwest::Client::new();
        let resp = client
            .get(format!("{}/files/{}", PAD_API_URL, r2_key))
            .bearer_auth(&self.token)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(anyhow!("Download failed ({}): {}", status, text));
        }

        let encrypted_data = resp.bytes().await?;
        decrypt_file(&encrypted_data, key)
    }
}

fn generate_ws_key() -> String {
    let mut key = [0u8; 16];
    rand::thread_rng().fill(&mut key);
    BASE64.encode(key)
}
