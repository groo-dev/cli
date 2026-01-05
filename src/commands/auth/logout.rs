use anyhow::Result;
use console::style;

use crate::auth::storage::AuthState;

pub fn run() -> Result<()> {
    match AuthState::load()? {
        Some(_) => {
            AuthState::clear()?;
            println!("{} Logged out successfully", style("✓").green());
        }
        None => {
            println!("Not currently logged in");
        }
    }

    Ok(())
}
