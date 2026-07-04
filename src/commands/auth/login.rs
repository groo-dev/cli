use anyhow::{anyhow, bail, Result};
use console::style;
use std::path::PathBuf;

use crate::auth::storage::{save_auth, AuthState};
use crate::auth::{accounts_url, CLIENT_ID, OAUTH_SCOPES};

pub async fn run(use_pat: bool, token_file: Option<PathBuf>) -> Result<()> {
    if let Some(path) = token_file {
        bail!(
            "--token-file is not supported on login: set GROO_TOKEN_FILE={} instead (it must be present for every command)",
            path.display()
        );
    }

    if use_pat {
        login_with_pat().await
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
    let user_email = validate_token(&token).await?;
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

async fn validate_token(token: &str) -> Result<String> {
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
    Ok(resp.user.email.or(resp.user.phone).unwrap_or_else(|| "unknown".to_string()))
}

async fn login_with_oauth() -> Result<()> {
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
    use sha2::{Digest, Sha256};
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    use tokio::net::TcpListener;

    const REDIRECT_PORT: u16 = 9876;

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

    // Start local server
    let listener = TcpListener::bind(format!("127.0.0.1:{}", REDIRECT_PORT)).await?;
    let redirect_uri = format!("http://127.0.0.1:{}/callback", REDIRECT_PORT);

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
        println!("{}", style("Could not open browser automatically").yellow());
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
    let tokens = exchange_code(&code, &code_verifier, &redirect_uri).await?;
    println!("{}", style("OK").green());

    // Get user info
    let user_email = validate_token(&tokens.access_token).await?;

    // Save auth state
    let auth = AuthState {
        access_token: tokens.access_token,
        refresh_token: tokens.refresh_token,
        token_type: "oauth".to_string(),
        expires_at: tokens.expires_at,
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

#[derive(serde::Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: Option<String>,
    expires_in: Option<i64>,
}

struct Tokens {
    access_token: String,
    refresh_token: Option<String>,
    expires_at: Option<i64>,
}

async fn exchange_code(code: &str, verifier: &str, redirect_uri: &str) -> Result<Tokens> {
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
        let text = resp.text().await.unwrap_or_default();
        return Err(anyhow!("Token exchange failed: {}", text));
    }

    let token_resp: TokenResponse = resp.json().await?;

    let expires_at = token_resp.expires_in.map(|secs| {
        chrono::Utc::now().timestamp() + secs
    });

    Ok(Tokens {
        access_token: token_resp.access_token,
        refresh_token: token_resp.refresh_token,
        expires_at,
    })
}
