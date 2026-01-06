use anyhow::{anyhow, Result};

use crate::auth::storage::AuthState;
use crate::pass::client::PassClient;
use crate::pass::tui;

pub async fn run() -> Result<()> {
    // Check auth
    let auth = AuthState::load()?.ok_or_else(|| {
        anyhow!("Not logged in. Run 'groo auth login' first.")
    })?;

    // Prompt for master password
    let password = rpassword::prompt_password("🔑 Master password: ")?;

    // Create client and unlock vault
    let client = PassClient::new(auth.access_token);
    let (vault, key, version) = client.unlock(&password).await?;

    // Launch TUI
    tui::run(vault, key, version).await
}
