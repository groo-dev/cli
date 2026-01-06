use anyhow::Result;
use console::style;

use crate::auth::storage::AuthState;

pub fn run() -> Result<()> {
    if AuthState::exists() {
        AuthState::clear()?;
        println!("{} Logged out successfully", style("✓").green());
    } else {
        println!("Not currently logged in");
    }

    Ok(())
}
