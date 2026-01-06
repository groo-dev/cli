use anyhow::Result;

use crate::auth::storage::load_auth_with_password;
use crate::pad::tui;

pub async fn run() -> Result<()> {
    // Check auth (prompts for master password)
    let (auth, _master_password) = load_auth_with_password()?;

    // Prompt for pad encryption password
    let password = rpassword::prompt_password("Pad encryption password: ")?;

    // Run the TUI
    tui::run(auth.access_token, &password).await
}
