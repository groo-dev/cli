use anyhow::Result;

use crate::pad::tui;

pub async fn run() -> Result<()> {
    // Prompt for pad encryption password
    let password = rpassword::prompt_password("Pad encryption password: ")?;

    // Run the TUI
    tui::run(&password).await
}
