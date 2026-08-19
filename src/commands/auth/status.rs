use std::time::SystemTime;

use anyhow::Result;
use console::style;

use crate::auth;

pub fn run() -> Result<()> {
    match auth::client()?.status()? {
        Some(metadata) => {
            println!("{} Logged in", style("✓").green());
            if let Some(email) = metadata.email {
                println!("  User: {}", style(email).cyan());
            }
            println!("  Auth type: OAuth");
            println!("  Storage: OS keyring");
            if let Some(expires_at) = metadata.expires_at {
                match expires_at.duration_since(SystemTime::now()) {
                    Ok(remaining) => println!(
                        "  Expires in: {}h {}m",
                        remaining.as_secs() / 3600,
                        (remaining.as_secs() % 3600) / 60
                    ),
                    Err(_) => println!("  Status: {}", style("Token expired").red()),
                }
            }
        }
        None => {
            println!("{} Not logged in", style("✗").red());
            println!("  Storage: OS keyring");
            println!("\nRun {} to authenticate", style("groo auth login").cyan());
        }
    }
    Ok(())
}
