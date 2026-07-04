//! Resolves a ready-to-use [`AuthState`] for every command, transparently
//! refreshing the OAuth access token when it is expired or close to it.

use anyhow::{bail, Context, Result};

use super::storage::{clear_auth, load_auth, save_auth, AuthState};
use super::{accounts_url, OAuthError, TokenResponse, CLIENT_ID};

/// Refresh a little before actual expiry so a token doesn't die mid-request.
const REFRESH_SKEW_SECS: i64 = 60;

/// Returns a ready-to-use auth state, refreshing the access token first when
/// it is expired or about to expire. PATs pass through untouched.
pub async fn get_valid_auth() -> Result<AuthState> {
    let Some(state) = load_auth()? else {
        bail!("not logged in — run 'groo auth login'");
    };
    if state.token_type != "oauth" {
        return Ok(state); // PATs don't expire client-side
    }
    if !needs_refresh(state.expires_at, chrono::Utc::now().timestamp()) {
        return Ok(state);
    }
    refresh(state).await
}

/// Pure refresh-decision boundary: given the stored expiry and the current
/// time (both unix seconds), should we refresh before returning the token?
/// A missing `expires_at` never triggers a proactive refresh.
fn needs_refresh(expires_at: Option<i64>, now: i64) -> bool {
    expires_at
        .map(|exp| exp - now < REFRESH_SKEW_SECS)
        .unwrap_or(false)
}

async fn refresh(state: AuthState) -> Result<AuthState> {
    let Some(refresh_token) = state.refresh_token.clone() else {
        bail!("access token expired and no refresh token is stored — run 'groo auth login'");
    };

    // Known limitation: refresh tokens rotate on use, and the server
    // revokes the old one. If two `groo` processes both decide to refresh
    // at nearly the same time, one wins the rotation and the other's
    // request replays an already-revoked refresh token. Most OAuth servers
    // treat that as reuse of a revoked token and revoke the whole token
    // family as a precaution, which logs both processes out. This is rare
    // in a CLI (commands are short-lived and mostly sequential), fails
    // closed, and is trivially recoverable via `groo auth login`, so no
    // cross-process locking is added for it here.
    let resp = reqwest::Client::new()
        .post(format!("{}/v1/oauth/token", accounts_url()))
        .form(&[
            ("grant_type", "refresh_token"),
            ("client_id", CLIENT_ID),
            ("refresh_token", refresh_token.as_str()),
        ])
        .send()
        .await
        .context("could not reach accounts to refresh the session")?;

    if !resp.status().is_success() {
        let err: OAuthError = resp.json().await.unwrap_or(OAuthError {
            error: "unknown".into(),
            error_description: None,
        });
        if err.error == "invalid_grant" {
            clear_auth()?;
            bail!("session revoked or expired — run 'groo auth login'");
        }
        bail!(
            "token refresh failed ({}): {}",
            err.error,
            err.error_description.unwrap_or_default()
        );
    }

    let tok: TokenResponse = resp.json().await.context("invalid token response")?;
    let new_state = AuthState {
        access_token: tok.access_token,
        // Persist the rotated refresh token; fall back to the old one only
        // if the server omitted a new one in the response.
        refresh_token: tok.refresh_token.or(Some(refresh_token)),
        token_type: "oauth".into(),
        expires_at: tok.expires_in.map(|s| chrono::Utc::now().timestamp() + s),
        user_email: state.user_email,
    };
    save_auth(&new_state)?;
    Ok(new_state)
}

#[cfg(test)]
mod tests {
    use super::needs_refresh;

    #[test]
    fn no_expiry_never_refreshes() {
        assert!(!needs_refresh(None, 1_000_000));
    }

    #[test]
    fn well_before_expiry_does_not_refresh() {
        assert!(!needs_refresh(Some(1_000_200), 1_000_000)); // 200s left
    }

    #[test]
    fn just_inside_skew_window_refreshes() {
        assert!(needs_refresh(Some(1_000_059), 1_000_000)); // 59s left
    }

    #[test]
    fn exactly_at_skew_boundary_does_not_refresh_yet() {
        // exp - now == REFRESH_SKEW_SECS is not `< skew`, so this is the
        // last moment we still serve the cached token.
        assert!(!needs_refresh(Some(1_000_060), 1_000_000));
    }

    #[test]
    fn already_expired_refreshes() {
        assert!(needs_refresh(Some(999_000), 1_000_000));
    }
}
