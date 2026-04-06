mod config;
mod state;
mod validation;

use anyhow::{bail, Result};
use std::path::PathBuf;

#[allow(unused_variables)]
pub async fn run(config_path: PathBuf, clean: bool) -> Result<()> {
    // Load and parse config
    let config_content = std::fs::read_to_string(&config_path)
        .map_err(|e| anyhow::anyhow!("Failed to read config file '{}': {}", config_path.display(), e))?;

    let config: config::LaunchpadConfig = serde_json::from_str(&config_content)
        .map_err(|e| anyhow::anyhow!("Invalid config JSON: {}", e))?;

    // Determine root directory
    let root = if config.root == "." {
        std::env::current_dir()?
    } else {
        std::env::current_dir()?.join(&config.root)
    };

    // Validate
    let errors = validation::validate(&config, &root);
    if !errors.is_empty() {
        let count = errors.len();
        let mut msg = format!("\n  Launchpad 🚀\n\n  ✗ Config validation failed ({} error{}):\n", count, if count == 1 { "" } else { "s" });
        for (i, error) in errors.iter().enumerate() {
            msg.push_str(&format!("\n  {}. {}\n", i + 1, error));
        }
        bail!("{}", msg);
    }

    println!("\n  Launchpad 🚀\n");
    println!("  Config validated. Pipeline not yet implemented.\n");

    Ok(())
}
