pub mod provider;

use anyhow::Result;
use groo_cli_auth::{AuthClient, AuthConfig, RefreshTokenPolicy};

const ACCOUNTS_URL: &str = "https://me.groo.dev";
// Registered under Pad, with sibling rows under Pass, Ops and Tasks all sharing
// `bundle_id = dev.groo.cli`. Any of the four works and they are
// interchangeable: `runtime` resolves the full application set from the row's
// bundle id, not from the id presented
// (runtime/api/src/services/application-set.service.ts).
//
// The previous value, app_54d1a63040d472d0cce73ac8cb2d61a3, belonged to the
// catch-all `Groo` application, DELETED on 2026-08-22 because it was the
// entitlement unit for all seven products. It took `Groo CLI` with it and left
// `groo auth login` failing with `unknown client_id`.
//
// KNOWN GAP -- `--device` still cannot work. The device-authorization endpoint
// resolves ONE application (`resolveApplicationOrFault`, oauth.ts:435) and never
// calls `resolveRequestableApplications`, so multi-application was built for
// /authorize only. Since OAUTH_SCOPES below spans four applications and is also
// passed as `required_scopes`, the device flow cannot satisfy it whichever
// sibling id is used here. The default browser/loopback login is unaffected.
const CLIENT_ID: &str = "client_5e13bc0b74b0ffa4c7a8b8790b6e13e5";
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

#[cfg(test)]
mod tests {
    use super::OAUTH_SCOPES;

    /// Every requested scope must sit inside some application registered for
    /// `bundle_id = dev.groo.cli`. Verified against live `tenant-v1` on
    /// 2026-08-25 that is Pad, Pass, Ops and Tasks, whose ceilings union to the
    /// set below.
    ///
    /// This is a login-or-not guard. `/authorize` rejects the WHOLE request with
    /// `invalid_scope` when one requested scope falls outside every named
    /// application's ceiling (runtime/api/src/routes/oauth.ts:190) — it does not
    /// drop the stray one. On top of that `required_scopes` is set to
    /// OAUTH_SCOPES, so a scope that is merely *ungranted* also fails the login
    /// client-side. Adding a scope here without registering a sibling client and
    /// granting entitlement breaks `groo auth login` outright.
    #[test]
    fn requests_no_scope_outside_the_applications_this_bundle_is_registered_for() {
        const REGISTERED: &[&str] = &[
            "openid", "profile", "email", "offline_access", "accounts:profile",
            "pad:read", "pad:write",
            "pass:read", "pass:write",
            "ops:read", "ops:write",
            "tasks:read", "tasks:write",
        ];

        let outside: Vec<&str> = OAUTH_SCOPES
            .iter()
            .copied()
            .filter(|s| !REGISTERED.contains(s))
            .collect();

        assert!(outside.is_empty(), "scopes outside every registered application: {outside:?}");
    }
}
