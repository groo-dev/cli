use anyhow::Result;
use console::style;

use crate::auth::storage::{backend_description, load_auth};

pub fn run() -> Result<()> {
    match load_auth()? {
        Some(auth) => {
            println!("{} Logged in", style("✓").green());

            if let Some(email) = &auth.user_email {
                println!("  User: {}", style(email).cyan());
            }

            println!("  Auth type: {}", auth.token_type);
            println!("  Storage: {}", backend_description());

            if let Some(expires_at) = auth.expires_at {
                let now = chrono::Utc::now().timestamp();
                if now >= expires_at {
                    println!("  Status: {}", style("Token expired").red());
                } else {
                    let remaining = expires_at - now;
                    let hours = remaining / 3600;
                    let mins = (remaining % 3600) / 60;
                    println!("  Expires in: {}h {}m", hours, mins);
                }
            } else if auth.token_type == "pat" {
                println!("  Expires: never (personal access token)");
            }
        }
        None => {
            println!("{} Not logged in", style("✗").red());
            println!("  Storage: {}", backend_description());
            println!("\nRun {} to authenticate", style("groo auth login").cyan());
        }
    }

    Ok(())
}
