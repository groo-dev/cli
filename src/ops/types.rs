use serde::{Deserialize, Serialize};

/// Application from ops API
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Application {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub repository: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// Config value (secret or variable)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigValue {
    pub id: String,
    pub application_id: String,
    pub environment: String,
    #[serde(rename = "type")]
    pub config_type: ConfigType,
    pub name: String,
    pub value: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConfigType {
    Secret,
    Variable,
}

/// Environment key (public key for secrets encryption)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvironmentKey {
    pub id: String,
    pub application_id: String,
    pub environment: String,
    pub public_key: String,
    pub created_at: String,
    pub updated_at: String,
}

// API Response types

#[derive(Debug, Deserialize)]
pub struct AppsResponse {
    pub apps: Vec<Application>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct AppResponse {
    pub app: Application,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigResponse {
    pub secrets: Vec<ConfigValue>,
    pub variables: Vec<ConfigValue>,
    pub secrets_enabled: bool,
    pub public_key: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ConfigCreateResponse {
    pub config: ConfigValue,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvironmentKeyResponse {
    pub environment_key: EnvironmentKey,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct SuccessResponse {
    pub success: bool,
}

#[derive(Debug, Deserialize)]
pub struct ErrorResponse {
    pub error: String,
    #[allow(dead_code)]
    pub code: String,
}

// Request types

#[derive(Debug, Serialize)]
pub struct EnableSecretsRequest {
    pub environment: String,
    #[serde(rename = "publicKey")]
    pub public_key: String,
}

#[derive(Debug, Serialize)]
pub struct ResetSecretsRequest {
    pub environment: String,
    #[serde(rename = "publicKey")]
    pub public_key: String,
}

#[derive(Debug, Serialize)]
pub struct CreateConfigRequest {
    #[serde(rename = "type")]
    pub config_type: ConfigType,
    pub environment: String,
    pub name: String,
    pub value: String,
}

#[derive(Debug, Serialize)]
pub struct UpdateConfigRequest {
    pub value: String,
}
