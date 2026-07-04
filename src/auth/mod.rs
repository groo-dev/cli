pub mod provider;
pub mod storage;

/// Default base URL of the Groo accounts service.
pub const ACCOUNTS_URL: &str = "https://accounts.groo.dev";

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
