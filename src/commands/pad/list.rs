use anyhow::{anyhow, Result};

use crate::auth::storage::AuthState;
use crate::pad::tui;

pub async fn run() -> Result<()> {
    // Check auth
    let auth = AuthState::load()?.ok_or_else(|| {
        anyhow!("Not logged in. Run 'groo auth login' first.")
    })?;

    // Prompt for encryption password
    let password = rpassword::prompt_password("Encryption password: ")?;

    // Run the TUI
    tui::run(auth.access_token, &password).await
}
