use anyhow::Result;
use console::style;

use crate::auth::storage::{clear_auth, has_stored_auth};

pub fn run() -> Result<()> {
    if has_stored_auth() {
        clear_auth()?;
        println!("{} Logged out successfully", style("✓").green());
    } else {
        println!("Not currently logged in");
    }

    Ok(())
}
