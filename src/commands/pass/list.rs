use anyhow::Result;

use crate::auth::provider;
use crate::pass::client::PassClient;
use crate::pass::tui;

pub async fn run() -> Result<()> {
    // Check auth
    provider::get_valid_auth().await?;
    let master_password = rpassword::prompt_password("🔑 Master password: ")?;

    // Create client and unlock vault
    let client = PassClient::new();
    let (vault, key, version) = client.unlock(&master_password).await?;

    // Launch TUI
    tui::run(vault, key, version).await
}
