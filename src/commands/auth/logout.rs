use anyhow::Result;
use console::style;

use crate::auth::storage::{clear_auth, load_auth};
use crate::auth::{accounts_url, CLIENT_ID};

pub async fn run() -> Result<()> {
    // `load_auth` (not `get_valid_auth`) — logout must not trigger a token
    // refresh just to figure out what to revoke. A read error here (corrupt
    // JSON, unreadable keyring, legacy on-disk format) is treated the same
    // as "nothing to revoke": `clear_auth` below is unconditional and
    // exhaustive, so local state gets wiped either way.
    let state = load_auth().ok().flatten();

    match &state {
        Some(state) if state.token_type == "oauth" => {
            if let Some(refresh_token) = &state.refresh_token {
                revoke_refresh_token(refresh_token).await;
            }
        }
        Some(state) if state.token_type == "pat" => {
            println!(
                "Personal access tokens aren't revoked by logout — revoke it from {} if it should no longer work.",
                style(format!("{}/settings", accounts_url())).cyan()
            );
        }
        _ => {}
    }

    // Unconditional: never gate this behind a "do we appear to have stored
    // auth?" check. `clear_auth` is exhaustive and idempotent (keyring entry
    // + GROO_TOKEN_FILE path + legacy files), so calling it even when
    // nothing looks stored is always safe — and it's the only way to catch
    // a keyring entry left behind from a session that ran without
    // GROO_TOKEN_FILE set, when the current session has it set (or vice
    // versa).
    clear_auth()?;

    println!("{} Logged out", style("✓").green());
    Ok(())
}

/// Best-effort refresh-token revocation via `POST /v1/oauth/revoke` (RFC
/// 7009). Public/native clients never hold a secret, so only `token` and
/// `client_id` are sent. Per RFC 7009 §2.2, the server returns 200 for both
/// a successfully revoked token and one it doesn't recognize — so any
/// non-2xx response or network failure is the only signal that revocation
/// didn't happen, and even then the caller still clears local credentials
/// and lets the refresh token expire on its own.
async fn revoke_refresh_token(refresh_token: &str) {
    let result = reqwest::Client::new()
        .post(format!("{}/v1/oauth/revoke", accounts_url()))
        .form(&[("token", refresh_token), ("client_id", CLIENT_ID)])
        .send()
        .await;

    let revoked = matches!(result, Ok(resp) if resp.status().is_success());
    if !revoked {
        println!(
            "{}",
            style("! could not reach accounts to revoke the session — local credentials cleared; the refresh token expires on its own")
                .yellow()
        );
    }
}
