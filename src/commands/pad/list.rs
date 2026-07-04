use anyhow::Result;

use crate::auth::provider;
use crate::pad::tui;

pub async fn run() -> Result<()> {
    let auth = provider::get_valid_auth().await?;

    // Prompt for pad encryption password
    let password = rpassword::prompt_password("Pad encryption password: ")?;

    // Run the TUI
    tui::run(auth.access_token, &password).await
}
