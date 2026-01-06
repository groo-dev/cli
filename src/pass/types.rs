use serde::{Deserialize, Serialize};

// =============================================================================
// API Response Types
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyInfoResponse {
    pub key_salt: String,
    pub kdf_iterations: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultResponse {
    pub encrypted_data: String,
    pub iv: String,
    pub version: u32,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultVersionResponse {
    pub version: u32,
    pub updated_at: i64,
}

// =============================================================================
// Vault Data Structure (decrypted client-side)
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Vault {
    pub version: u32,
    pub items: Vec<VaultItem>,
    pub folders: Vec<Folder>,
    pub last_modified: i64,
    /// RSA private key for sharing (JWK format, stored encrypted in vault)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rsa_private_key: Option<String>,
}

// =============================================================================
// Vault Items
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum VaultItem {
    Password(PasswordItem),
    Passkey(PasskeyItem),
    Note(NoteItem),
    Card(CardItem),
    BankAccount(BankAccountItem),
    File(FileItem),
}

impl VaultItem {
    pub fn id(&self) -> &str {
        match self {
            VaultItem::Password(item) => &item.id,
            VaultItem::Passkey(item) => &item.id,
            VaultItem::Note(item) => &item.id,
            VaultItem::Card(item) => &item.id,
            VaultItem::BankAccount(item) => &item.id,
            VaultItem::File(item) => &item.id,
        }
    }

    pub fn name(&self) -> &str {
        match self {
            VaultItem::Password(item) => &item.name,
            VaultItem::Passkey(item) => &item.name,
            VaultItem::Note(item) => &item.name,
            VaultItem::Card(item) => &item.name,
            VaultItem::BankAccount(item) => &item.name,
            VaultItem::File(item) => &item.name,
        }
    }

    pub fn deleted_at(&self) -> Option<i64> {
        match self {
            VaultItem::Password(item) => item.deleted_at,
            VaultItem::Passkey(item) => item.deleted_at,
            VaultItem::Note(item) => item.deleted_at,
            VaultItem::Card(item) => item.deleted_at,
            VaultItem::BankAccount(item) => item.deleted_at,
            VaultItem::File(item) => item.deleted_at,
        }
    }

    pub fn type_icon(&self) -> &'static str {
        match self {
            VaultItem::Password(_) => "🔐",
            VaultItem::Passkey(_) => "🔑",
            VaultItem::Note(_) => "📝",
            VaultItem::Card(_) => "💳",
            VaultItem::BankAccount(_) => "🏦",
            VaultItem::File(_) => "📎",
        }
    }

    pub fn type_label(&self) -> &'static str {
        match self {
            VaultItem::Password(_) => "Password",
            VaultItem::Passkey(_) => "Passkey",
            VaultItem::Note(_) => "Note",
            VaultItem::Card(_) => "Card",
            VaultItem::BankAccount(_) => "Bank Account",
            VaultItem::File(_) => "File",
        }
    }
}

// =============================================================================
// Password Item
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PasswordItem {
    pub id: String,
    pub name: String,
    pub username: String,
    pub password: String,
    #[serde(default)]
    pub urls: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub totp: Option<TotpConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub folder_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub favorite: Option<bool>,
    pub created_at: i64,
    pub updated_at: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deleted_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TotpConfig {
    pub secret: String,
    pub algorithm: TotpAlgorithm,
    pub digits: u8,
    pub period: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TotpAlgorithm {
    SHA1,
    SHA256,
    SHA512,
}

// =============================================================================
// Passkey Item
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PasskeyItem {
    pub id: String,
    pub name: String,
    pub rp_id: String,
    pub rp_name: String,
    pub credential_id: String,
    pub public_key: String,
    pub private_key: String,
    pub user_handle: String,
    pub user_name: String,
    pub sign_count: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub folder_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub favorite: Option<bool>,
    pub created_at: i64,
    pub updated_at: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deleted_at: Option<i64>,
}

// =============================================================================
// Note Item
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteItem {
    pub id: String,
    pub name: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub folder_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub favorite: Option<bool>,
    pub created_at: i64,
    pub updated_at: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deleted_at: Option<i64>,
}

// =============================================================================
// Card Item
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CardItem {
    pub id: String,
    pub name: String,
    pub cardholder_name: String,
    pub number: String,
    pub exp_month: String,
    pub exp_year: String,
    pub cvv: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub brand: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub folder_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub favorite: Option<bool>,
    pub created_at: i64,
    pub updated_at: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deleted_at: Option<i64>,
}

// =============================================================================
// Bank Account Item
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BankAccountItem {
    pub id: String,
    pub name: String,
    pub bank_name: String,
    pub account_type: BankAccountType,
    pub account_number: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub routing_number: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub iban: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub swift_bic: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub folder_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub favorite: Option<bool>,
    pub created_at: i64,
    pub updated_at: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deleted_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BankAccountType {
    Checking,
    Savings,
    Other,
}

// =============================================================================
// File Item
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileItem {
    pub id: String,
    pub name: String,
    pub file_name: String,
    pub file_size: u64,
    pub mime_type: String,
    pub r2_key: String,
    pub encryption_iv: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub folder_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub favorite: Option<bool>,
    pub created_at: i64,
    pub updated_at: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deleted_at: Option<i64>,
}

// =============================================================================
// Folder
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Folder {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
}
