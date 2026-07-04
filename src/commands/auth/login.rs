use anyhow::{anyhow, bail, Context, Result};
use console::style;
use std::path::PathBuf;

use crate::auth::storage::{save_auth, AuthState};
use crate::auth::{accounts_url, OAuthError, TokenResponse, CLIENT_ID, OAUTH_SCOPES};

pub async fn run(use_pat: bool, use_device: bool, token_file: Option<PathBuf>) -> Result<()> {
    if let Some(path) = token_file {
        bail!(
            "--token-file is not supported on login: set GROO_TOKEN_FILE={} instead (it must be present for every command)",
            path.display()
        );
    }

    if use_pat && use_device {
        bail!("--pat and --device are mutually exclusive");
    }

    if use_pat {
        login_with_pat().await
    } else if use_device {
        login_with_device().await
    } else {
        login_with_oauth().await
    }
}

async fn login_with_pat() -> Result<()> {
    let token = if atty::is(atty::Stream::Stdin) {
        // Interactive mode - prompt for token
        println!("Login with Personal Access Token");
        println!(
            "Create a token at: {}\n",
            style("https://accounts.groo.dev/settings").cyan()
        );
        rpassword::prompt_password("Paste your token: ")?
    } else {
        // Piped input - read from stdin
        use std::io::Read;
        let mut buffer = String::new();
        std::io::stdin().read_to_string(&mut buffer)?;
        buffer
    };
    let token = token.trim().to_string();

    if !token.starts_with("groo_pat_") {
        return Err(anyhow!(
            "Invalid token format. Token should start with 'groo_pat_'"
        ));
    }

    // Validate token by fetching user info
    print!("Validating token... ");
    let user_email = validate_pat(&token).await?;
    println!("{}", style("OK").green());

    // Save auth state
    let auth = AuthState {
        access_token: token,
        refresh_token: None,
        token_type: "pat".to_string(),
        expires_at: None,
        user_email: Some(user_email.clone()),
    };
    save_auth(&auth)?;

    println!(
        "\n{} Logged in as {}",
        style("✓").green(),
        style(&user_email).cyan()
    );

    Ok(())
}

/// Validates a Personal Access Token via `/v1/auth/me`. This endpoint only
/// accepts session cookies or `groo_pat_`-prefixed tokens — it is not used
/// for OAuth access tokens, which go through `/v1/oauth/userinfo` instead
/// (see `fetch_userinfo_email`).
async fn validate_pat(token: &str) -> Result<String> {
    let client = reqwest::Client::new();
    let resp = client
        .get(format!("{}/v1/auth/me", accounts_url()))
        .bearer_auth(token)
        .send()
        .await?;

    if !resp.status().is_success() {
        return Err(anyhow!("Invalid token"));
    }

    #[derive(serde::Deserialize)]
    struct ApiResponse {
        user: UserInfo,
    }

    #[derive(serde::Deserialize)]
    struct UserInfo {
        email: Option<String>,
        phone: Option<String>,
    }

    let resp: ApiResponse = resp.json().await?;
    Ok(resp
        .user
        .email
        .or(resp.user.phone)
        .unwrap_or_else(|| "unknown".to_string()))
}

async fn login_with_oauth() -> Result<()> {
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
    use sha2::{Digest, Sha256};
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    use tokio::net::TcpListener;

    println!("Login with Groo Account\n");

    // Generate PKCE code verifier and challenge
    let code_verifier: String = {
        let bytes: [u8; 32] = rand::random();
        URL_SAFE_NO_PAD.encode(bytes)
    };
    let code_challenge = {
        let hash = Sha256::digest(code_verifier.as_bytes());
        URL_SAFE_NO_PAD.encode(hash)
    };
    let state: String = {
        let bytes: [u8; 16] = rand::random();
        URL_SAFE_NO_PAD.encode(bytes)
    };

    // Bind to an ephemeral port: a fixed port risks colliding with another
    // process (or a second `groo auth login` run), and needs no firewall
    // exception beyond loopback.
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let port = listener.local_addr()?.port();
    let redirect_uri = format!("http://127.0.0.1:{port}/callback");

    // Build auth URL
    let auth_url = format!(
        "{}/v1/oauth/authorize?client_id={}&redirect_uri={}&response_type=code&state={}&code_challenge={}&code_challenge_method=S256&scope={}",
        accounts_url(),
        CLIENT_ID,
        urlencoding::encode(&redirect_uri),
        state,
        code_challenge,
        urlencoding::encode(OAUTH_SCOPES),
    );

    println!("Opening browser for authentication...");
    println!(
        "If the browser doesn't open, visit:\n{}\n",
        style(&auth_url).dim()
    );

    if open::that(&auth_url).is_err() {
        println!(
            "{}",
            style("Could not open browser automatically.").yellow()
        );
        println!(
            "Visit this URL to continue:\n{}\n",
            style(&auth_url).cyan().bold()
        );
    }

    println!("Waiting for authentication...");

    // Wait for callback
    let (mut socket, _) = listener.accept().await?;
    let mut reader = BufReader::new(&mut socket);
    let mut request_line = String::new();
    reader.read_line(&mut request_line).await?;

    // Parse the request to extract code and state
    let code = parse_callback(&request_line, &state)?;

    // Send response to browser
    let response = "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n\r\n<html><body><h1>Success!</h1><p>You can close this window.</p><script>window.close()</script></body></html>";
    socket.write_all(response.as_bytes()).await?;
    drop(socket);

    // Exchange code for tokens
    print!("Exchanging code for tokens... ");
    let tok = exchange_code(&code, &code_verifier, &redirect_uri).await?;
    println!("{}", style("OK").green());

    finish_login(tok).await
}

fn parse_callback(request_line: &str, expected_state: &str) -> Result<String> {
    // Parse: GET /callback?code=xxx&state=yyy HTTP/1.1
    let path = request_line
        .split_whitespace()
        .nth(1)
        .ok_or_else(|| anyhow!("Invalid request"))?;

    let url = url::Url::parse(&format!("http://localhost{}", path))?;
    let params: std::collections::HashMap<_, _> = url.query_pairs().collect();

    let state = params
        .get("state")
        .ok_or_else(|| anyhow!("Missing state parameter"))?;
    if state != expected_state {
        return Err(anyhow!("State mismatch - possible CSRF attack"));
    }

    let code = params
        .get("code")
        .ok_or_else(|| anyhow!("Missing code parameter"))?;

    Ok(code.to_string())
}

async fn exchange_code(code: &str, verifier: &str, redirect_uri: &str) -> Result<TokenResponse> {
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{}/v1/oauth/token", accounts_url()))
        .form(&[
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", redirect_uri),
            ("client_id", CLIENT_ID),
            ("code_verifier", verifier),
        ])
        .send()
        .await?;

    if !resp.status().is_success() {
        let err: OAuthError = resp
            .json()
            .await
            .context("invalid error response from token endpoint")?;
        bail!(
            "token exchange failed ({}): {}",
            err.error,
            err.error_description.unwrap_or_default()
        );
    }

    resp.json().await.context("invalid token response")
}

/// RFC 8628 device authorization response, from `POST
/// /v1/oauth/device_authorization`.
#[derive(serde::Deserialize)]
struct DeviceAuthResponse {
    device_code: String,
    user_code: String,
    verification_uri: String,
    verification_uri_complete: String,
    expires_in: u64,
    /// Minimum poll interval in seconds. RFC 8628 §3.2 makes this OPTIONAL
    /// with a default of 5 — tolerate servers that omit it.
    #[serde(default = "default_device_interval")]
    interval: u64,
}

fn default_device_interval() -> u64 {
    5
}

/// `groo auth login --device`: RFC 8628 device authorization flow for
/// environments with no local browser to redirect back to (SSH sessions,
/// containers, etc). The user approves the login on another device; this
/// process polls the token endpoint until it's approved, denied, or expires.
async fn login_with_device() -> Result<()> {
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{}/v1/oauth/device_authorization", accounts_url()))
        .form(&[("client_id", CLIENT_ID), ("scope", OAUTH_SCOPES)])
        .send()
        .await?;

    if !resp.status().is_success() {
        let err: OAuthError = resp
            .json()
            .await
            .context("invalid error response from device authorization endpoint")?;
        bail!(
            "device authorization request failed ({}): {}",
            err.error,
            err.error_description.unwrap_or_default()
        );
    }
    let dev: DeviceAuthResponse = resp
        .json()
        .await
        .context("invalid device authorization response")?;

    println!(
        "\n  Visit:  {}",
        style(&dev.verification_uri_complete).cyan().bold()
    );
    println!("  Code:   {}\n", style(&dev.user_code).bold());
    println!(
        "  (If that link doesn't open, visit {} and enter the code above.)",
        dev.verification_uri
    );
    println!(
        "\nWaiting for approval (expires in {} min)...",
        dev.expires_in / 60
    );

    let mut interval = dev.interval.max(1);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(dev.expires_in);

    loop {
        if std::time::Instant::now() >= deadline {
            bail!("device code expired — run 'groo auth login --device' again");
        }
        tokio::time::sleep(std::time::Duration::from_secs(interval)).await;

        let resp = client
            .post(format!("{}/v1/oauth/token", accounts_url()))
            .form(&[
                (
                    "grant_type",
                    "urn:ietf:params:oauth:grant-type:device_code",
                ),
                ("client_id", CLIENT_ID),
                ("device_code", dev.device_code.as_str()),
            ])
            .send()
            .await?;

        if resp.status().is_success() {
            let tok: TokenResponse = resp.json().await.context("invalid token response")?;
            return finish_login(tok).await;
        }

        let err: OAuthError = resp
            .json()
            .await
            .context("invalid error response from token endpoint")?;
        match next_poll_action(&err.error, interval) {
            PollAction::Continue(new_interval) => {
                interval = new_interval;
                continue;
            }
            PollAction::Expired => {
                bail!("device code expired — run 'groo auth login --device' again")
            }
            PollAction::Denied => bail!("request denied on the approval page"),
            PollAction::Unknown => bail!(
                "device login failed ({}): {}",
                err.error,
                err.error_description.unwrap_or_default()
            ),
        }
    }
}

/// What to do next in the device-flow poll loop, given the token endpoint's
/// error code and the interval currently in use.
#[derive(Debug, PartialEq, Eq)]
enum PollAction {
    /// Keep polling, sleeping this many seconds before the next request.
    Continue(u64),
    /// The device code expired — the user must restart the flow.
    Expired,
    /// The user declined the request on the approval page.
    Denied,
    /// Any other error code — bail with the server's description.
    Unknown,
}

/// Pure decision boundary for RFC 8628 device-flow polling (§3.5): given the
/// `error` field from a token-endpoint error response and the poll interval
/// in effect, decide what happens next. Kept free of I/O and sleeping so it
/// can be exercised directly in unit tests below.
fn next_poll_action(error: &str, interval: u64) -> PollAction {
    match error {
        "authorization_pending" => PollAction::Continue(interval),
        "slow_down" => PollAction::Continue(interval + 5),
        "expired_token" => PollAction::Expired,
        "access_denied" => PollAction::Denied,
        _ => PollAction::Unknown,
    }
}

/// Shared success path for both the loopback and device OAuth flows: save
/// the new tokens FIRST, then resolve the user's email via
/// `/v1/oauth/userinfo` (the access token carries `openid`+`email` scope,
/// not a PAT, so `/v1/auth/me` won't accept it) as best-effort decoration.
/// Once the token exchange has succeeded, login must succeed: a transient
/// userinfo failure must never discard freshly issued tokens (for
/// `--device` that would mean redoing the whole human approval).
async fn finish_login(tok: TokenResponse) -> Result<()> {
    let expires_at = tok
        .expires_in
        .map(|secs| chrono::Utc::now().timestamp() + secs);

    let mut auth = AuthState {
        access_token: tok.access_token,
        refresh_token: tok.refresh_token,
        token_type: "oauth".to_string(),
        expires_at,
        user_email: None,
    };
    save_auth(&auth)?;

    // Best-effort: decorate the stored state with the user's email. Failing
    // here only costs the "Logged in as <email>" nicety, never the login.
    let user_email = match fetch_userinfo_email(&auth.access_token).await {
        Ok(email) => {
            auth.user_email = Some(email.clone());
            save_auth(&auth)?;
            Some(email)
        }
        Err(_) => {
            println!(
                "{}",
                style("! could not fetch your profile (login still succeeded)").yellow()
            );
            None
        }
    };

    if let Some(scope) = &tok.scope {
        println!("Granted scopes: {}", style(scope).dim());
    }
    match &user_email {
        Some(email) => println!(
            "\n{} Logged in as {}",
            style("✓").green(),
            style(email).cyan()
        ),
        None => println!("\n{} Logged in", style("✓").green()),
    }

    Ok(())
}

/// Resolves the logged-in user's email for a fresh OAuth access token via
/// the dedicated OAuth userinfo endpoint (RFC-ish `/v1/oauth/userinfo`).
/// Unlike `/v1/auth/me`, this accepts OAuth access tokens (not PATs) and
/// only returns `email` when the `email` scope was granted, which
/// `OAUTH_SCOPES` always requests.
async fn fetch_userinfo_email(token: &str) -> Result<String> {
    #[derive(serde::Deserialize)]
    struct UserInfo {
        #[serde(default)]
        email: Option<String>,
    }

    let client = reqwest::Client::new();
    let resp = client
        .get(format!("{}/v1/oauth/userinfo", accounts_url()))
        .bearer_auth(token)
        .send()
        .await?
        .error_for_status()
        .context("failed to fetch user info")?;

    let info: UserInfo = resp.json().await.context("invalid userinfo response")?;
    Ok(info.email.unwrap_or_else(|| "unknown".to_string()))
}

#[cfg(test)]
mod device_flow_tests {
    use super::*;

    #[test]
    fn authorization_pending_continues_with_unchanged_interval() {
        assert_eq!(
            next_poll_action("authorization_pending", 5),
            PollAction::Continue(5)
        );
    }

    #[test]
    fn slow_down_backs_off_by_five_seconds() {
        assert_eq!(next_poll_action("slow_down", 5), PollAction::Continue(10));
    }

    #[test]
    fn expired_token_bails() {
        assert_eq!(next_poll_action("expired_token", 5), PollAction::Expired);
    }

    #[test]
    fn access_denied_bails() {
        assert_eq!(next_poll_action("access_denied", 5), PollAction::Denied);
    }

    #[test]
    fn unrecognized_error_falls_through_to_unknown() {
        assert_eq!(next_poll_action("temporarily_unavailable", 5), PollAction::Unknown);
    }

    #[test]
    fn device_auth_response_interval_defaults_to_five_when_omitted() {
        // RFC 8628 §3.2: `interval` is OPTIONAL, default 5 seconds.
        let json = r#"{
            "device_code": "dc",
            "user_code": "BCDF-GHJK",
            "verification_uri": "https://accounts.groo.dev/device",
            "verification_uri_complete": "https://accounts.groo.dev/device?user_code=BCDF-GHJK",
            "expires_in": 600
        }"#;
        let dev: DeviceAuthResponse = serde_json::from_str(json).unwrap();
        assert_eq!(dev.interval, 5);
    }

    #[test]
    fn device_auth_response_uses_server_interval_when_present() {
        let json = r#"{
            "device_code": "dc",
            "user_code": "BCDF-GHJK",
            "verification_uri": "https://accounts.groo.dev/device",
            "verification_uri_complete": "https://accounts.groo.dev/device?user_code=BCDF-GHJK",
            "expires_in": 600,
            "interval": 7
        }"#;
        let dev: DeviceAuthResponse = serde_json::from_str(json).unwrap();
        assert_eq!(dev.interval, 7);
    }
}
