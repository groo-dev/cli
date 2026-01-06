use anyhow::Result;

use crate::auth::storage::load_auth_with_password;
use crate::pass::client::PassClient;
use crate::pass::tui;

pub async fn run() -> Result<()> {
    // Check auth (prompts for master password)
    let (auth, master_password) = load_auth_with_password()?;

    // Create client and unlock vault
    let client = PassClient::new(auth.access_token);
    let (vault, key, version) = client.unlock(&master_password).await?;

    // Launch TUI
    tui::run(vault, key, version).await
}
