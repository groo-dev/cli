use anyhow::Result;
use console::style;

use crate::auth;

pub async fn run() -> Result<()> {
    auth::client()?.logout().await?;
    println!("{} Logged out", style("✓").green());
    Ok(())
}
