use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedPayload {
    pub ciphertext: String, // base64
    pub iv: String,         // base64
    pub version: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileAttachment {
    pub id: String,
    pub encrypted_name: EncryptedPayload,
    pub encrypted_type: EncryptedPayload,
    pub size: u64,
    pub r2_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListItem {
    pub id: String,
    pub encrypted_text: EncryptedPayload,
    pub files: Vec<FileAttachment>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserState {
    pub active_id: String,
    pub list: Vec<ListItem>,
    pub encryption_salt: Option<String>,
    pub encryption_test: Option<EncryptedPayload>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ServerMessage {
    #[serde(rename = "sync")]
    Sync { state: UserState },
    #[serde(rename = "list:added")]
    ListAdded { item: ListItem },
    #[serde(rename = "list:deleted")]
    ListDeleted { id: String },
    #[serde(rename = "error")]
    Error { message: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ClientMessage {
    #[serde(rename = "list:add")]
    ListAdd { item: ListItem },
    #[serde(rename = "list:delete")]
    ListDelete { id: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileUploadResponse {
    pub id: String,
    pub size: u64,
    #[serde(rename = "r2Key")]
    pub r2_key: String,
}
