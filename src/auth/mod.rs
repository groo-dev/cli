pub mod provider;
pub mod storage;

/// Default base URL of the Groo accounts service.
pub const ACCOUNTS_URL: &str = "https://accounts.groo.dev";

/// Shared shape of a successful `POST /v1/oauth/token` response, common to
/// every grant type (authorization_code, refresh_token, device_code). Used
/// by both the token-refresh path (`provider`) and the login flows
/// (`commands::auth::login`) so the fields aren't defined twice.
#[derive(serde::Deserialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_in: Option<i64>,
    /// Space-delimited granted scopes, echoed back by the server. Present on
    /// every grant type but kept optional in case an older server omits it.
    #[serde(default)]
    pub scope: Option<String>,
}

/// Shared shape of an OAuth error response body, e.g.
/// `{"error": "invalid_grant", "error_description": "..."}`.
#[derive(serde::Deserialize)]
pub struct OAuthError {
    pub error: String,
    #[serde(default)]
    pub error_description: Option<String>,
}

/// Registered native/public OAuth client id for the CLI. Public clients
/// never hold a secret — PKCE covers the authorization code exchange.
pub const CLIENT_ID: &str = "app_54d1a63040d472d0cce73ac8cb2d61a3";

/// Scopes requested during OAuth login. `offline_access` is required to get
/// back a refresh token at all.
pub const OAUTH_SCOPES: &str =
    "openid profile email offline_access pass:read pass:write ops:read ops:write tasks:read tasks:write pad:read pad:write";

/// Resolves the accounts base URL, honoring `GROO_ACCOUNTS_URL` for local
/// dev / staging so every call site (login, refresh, token validation)
/// points at the same place.
pub fn accounts_url() -> std::borrow::Cow<'static, str> {
    match std::env::var("GROO_ACCOUNTS_URL") {
        Ok(v) if !v.is_empty() => std::borrow::Cow::Owned(v),
        _ => std::borrow::Cow::Borrowed(ACCOUNTS_URL),
    }
}
