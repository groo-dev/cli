pub mod provider;

use anyhow::Result;
use groo_cli_auth::{AuthClient, AuthConfig, RefreshTokenPolicy};

const ACCOUNTS_URL: &str = "https://me.groo.dev";
const CLIENT_ID: &str = "app_54d1a63040d472d0cce73ac8cb2d61a3";
const OAUTH_SCOPES: &[&str] = &[
    "openid",
    "profile",
    "email",
    "offline_access",
    "pass:read",
    "pass:write",
    "ops:read",
    "ops:write",
    "tasks:read",
    "tasks:write",
    "pad:read",
    "pad:write",
];

pub fn client() -> Result<AuthClient> {
    let base = std::env::var("GROO_ACCOUNTS_URL").unwrap_or_else(|_| ACCOUNTS_URL.to_owned());
    let base = base.trim_end_matches('/');
    let config = AuthConfig::builder()
        .client_id(CLIENT_ID)
        .authorization_endpoint(format!("{base}/v1/oauth/authorize"))?
        .token_endpoint(format!("{base}/v1/oauth/token"))?
        .device_authorization_endpoint(format!("{base}/v1/oauth/device_authorization"))?
        .revocation_endpoint(format!("{base}/v1/oauth/revoke"))?
        .userinfo_endpoint(format!("{base}/v1/oauth/userinfo"))?
        .scopes(OAUTH_SCOPES.iter().copied())
        .required_scopes(OAUTH_SCOPES.iter().copied())
        .require_userinfo_subject(true)
        .require_userinfo_email(true)
        .keyring_service("groo-cli")
        .keyring_account("default")
        .refresh_token_policy(RefreshTokenPolicy::RequireRotation)
        .build()?;
    Ok(AuthClient::new(config)?)
}
