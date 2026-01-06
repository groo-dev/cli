use anyhow::Result;
use console::style;

use crate::auth::storage::AuthState;

pub fn run() -> Result<()> {
    if !AuthState::exists() {
        println!("{} Not logged in", style("✗").red());
        println!("\nRun {} to authenticate", style("groo auth login").cyan());
        return Ok(());
    }

    let master_password = rpassword::prompt_password("🔑 Master password: ")?;

    match AuthState::load(&master_password)? {
        Some(auth) => {
            println!("{} Logged in", style("✓").green());

            if let Some(email) = &auth.user_email {
                println!("  User: {}", style(email).cyan());
            }

            println!("  Auth type: {}", auth.token_type);

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
            }
        }
        None => {
            println!("{} Not logged in", style("✗").red());
            println!("\nRun {} to authenticate", style("groo auth login").cyan());
        }
    }

    Ok(())
}
