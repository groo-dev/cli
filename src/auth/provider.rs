use anyhow::Result;

pub struct AuthState {
    pub access_token: String,
}

pub async fn get_valid_auth() -> Result<AuthState> {
    let token = super::client()?.access_token().await?;
    Ok(AuthState {
        access_token: token.expose_secret().to_owned(),
    })
}
