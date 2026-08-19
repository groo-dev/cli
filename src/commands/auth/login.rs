use anyhow::{Context, Result};
use console::style;

use crate::auth;

pub async fn run(use_device: bool) -> Result<()> {
    let client = auth::client()?;
    println!("Login with Groo Account\n");

    let metadata = if use_device {
        client
            .login_with_device(|prompt| {
                let url = prompt
                    .verification_uri_complete
                    .as_ref()
                    .unwrap_or(&prompt.verification_uri);
                println!("  Visit:  {}", style(url).cyan().bold());
                println!("  Code:   {}\n", style(&prompt.user_code).bold());
                println!(
                    "Waiting for approval (expires in {} min)...",
                    prompt.expires_in.as_secs() / 60
                );
            })
            .await?
    } else {
        println!("Opening browser for authentication...");
        println!("Waiting for authentication...");
        client
            .login_with_browser(&|url: &url::Url| open::that(url.as_str()))
            .await?
    };

    println!(
        "Granted scopes: {}",
        style(metadata.granted_scopes.join(" ")).dim()
    );
    let email = metadata
        .email
        .context("validated login metadata unexpectedly omitted email")?;
    println!(
        "\n{} Logged in as {}",
        style("✓").green(),
        style(email).cyan()
    );
    Ok(())
}
