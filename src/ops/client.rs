use anyhow::{Context, Result};
use reqwest::Client;

use super::types::*;

const OPS_API_URL: &str = "https://ops.groo.dev";

pub struct OpsClient {
    client: Client,
    token: String,
}

impl OpsClient {
    pub fn new(token: String) -> Self {
        Self {
            client: Client::new(),
            token,
        }
    }

    /// List all applications
    pub async fn list_apps(&self) -> Result<Vec<Application>> {
        let response = self
            .client
            .get(format!("{}/v1/apps", OPS_API_URL))
            .header("Cookie", format!("session={}", self.token))
            .send()
            .await
            .context("Failed to fetch apps")?;

        if !response.status().is_success() {
            let error: ErrorResponse = response.json().await?;
            anyhow::bail!("{}", error.error);
        }

        let data: AppsResponse = response.json().await?;
        Ok(data.apps)
    }

    /// Get a single application by ID
    #[allow(dead_code)]
    pub async fn get_app(&self, app_id: &str) -> Result<Application> {
        let response = self
            .client
            .get(format!("{}/v1/apps/{}", OPS_API_URL, app_id))
            .header("Cookie", format!("session={}", self.token))
            .send()
            .await
            .context("Failed to fetch app")?;

        if !response.status().is_success() {
            let error: ErrorResponse = response.json().await?;
            anyhow::bail!("{}", error.error);
        }

        let data: AppResponse = response.json().await?;
        Ok(data.app)
    }

    /// Get config for an application and environment
    pub async fn get_config(&self, app_id: &str, environment: &str) -> Result<ConfigResponse> {
        let response = self
            .client
            .get(format!(
                "{}/v1/apps/{}/config?environment={}",
                OPS_API_URL, app_id, environment
            ))
            .header("Cookie", format!("session={}", self.token))
            .send()
            .await
            .context("Failed to fetch config")?;

        if !response.status().is_success() {
            let error: ErrorResponse = response.json().await?;
            anyhow::bail!("{}", error.error);
        }

        response.json().await.context("Failed to parse config response")
    }

    /// Create a new config value (secret or variable)
    pub async fn create_config(
        &self,
        app_id: &str,
        request: CreateConfigRequest,
    ) -> Result<ConfigValue> {
        let response = self
            .client
            .post(format!("{}/v1/apps/{}/config", OPS_API_URL, app_id))
            .header("Cookie", format!("session={}", self.token))
            .json(&request)
            .send()
            .await
            .context("Failed to create config")?;

        if !response.status().is_success() {
            let error: ErrorResponse = response.json().await?;
            anyhow::bail!("{}", error.error);
        }

        let data: ConfigCreateResponse = response.json().await?;
        Ok(data.config)
    }

    /// Update an existing config value
    pub async fn update_config(
        &self,
        app_id: &str,
        config_id: &str,
        value: String,
    ) -> Result<ConfigValue> {
        let request = UpdateConfigRequest { value };
        let response = self
            .client
            .put(format!(
                "{}/v1/apps/{}/config/{}",
                OPS_API_URL, app_id, config_id
            ))
            .header("Cookie", format!("session={}", self.token))
            .json(&request)
            .send()
            .await
            .context("Failed to update config")?;

        if !response.status().is_success() {
            let error: ErrorResponse = response.json().await?;
            anyhow::bail!("{}", error.error);
        }

        let data: ConfigCreateResponse = response.json().await?;
        Ok(data.config)
    }

    /// Delete a config value
    #[allow(dead_code)]
    pub async fn delete_config(&self, app_id: &str, config_id: &str) -> Result<()> {
        let response = self
            .client
            .delete(format!(
                "{}/v1/apps/{}/config/{}",
                OPS_API_URL, app_id, config_id
            ))
            .header("Cookie", format!("session={}", self.token))
            .send()
            .await
            .context("Failed to delete config")?;

        if !response.status().is_success() {
            let error: ErrorResponse = response.json().await?;
            anyhow::bail!("{}", error.error);
        }

        Ok(())
    }

    /// Enable secrets for an environment (store public key)
    pub async fn enable_secrets(
        &self,
        app_id: &str,
        environment: &str,
        public_key: &str,
    ) -> Result<EnvironmentKey> {
        let request = EnableSecretsRequest {
            environment: environment.to_string(),
            public_key: public_key.to_string(),
        };

        let response = self
            .client
            .post(format!("{}/v1/apps/{}/secrets/enable", OPS_API_URL, app_id))
            .header("Cookie", format!("session={}", self.token))
            .json(&request)
            .send()
            .await
            .context("Failed to enable secrets")?;

        if !response.status().is_success() {
            let error: ErrorResponse = response.json().await?;
            anyhow::bail!("{}", error.error);
        }

        let data: EnvironmentKeyResponse = response.json().await?;
        Ok(data.environment_key)
    }

    /// Reset secrets for an environment (delete all secrets and update public key)
    pub async fn reset_secrets(
        &self,
        app_id: &str,
        environment: &str,
        public_key: &str,
    ) -> Result<()> {
        let request = ResetSecretsRequest {
            environment: environment.to_string(),
            public_key: public_key.to_string(),
        };

        let response = self
            .client
            .post(format!("{}/v1/apps/{}/secrets/reset", OPS_API_URL, app_id))
            .header("Cookie", format!("session={}", self.token))
            .json(&request)
            .send()
            .await
            .context("Failed to reset secrets")?;

        if !response.status().is_success() {
            let error: ErrorResponse = response.json().await?;
            anyhow::bail!("{}", error.error);
        }

        Ok(())
    }
}
