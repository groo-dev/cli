use anyhow::{anyhow, Result};
use console::style;

use crate::auth::storage::AuthState;
use crate::pass::client::PassClient;
use crate::pass::types::VaultItem;

pub async fn run(query: &str, username: bool, totp: bool, show: bool) -> Result<()> {
    // Check auth
    let auth = AuthState::load()?.ok_or_else(|| {
        anyhow!("Not logged in. Run 'groo auth login' first.")
    })?;

    // Prompt for master password
    let password = rpassword::prompt_password("🔑 Master password: ")?;

    // Create client and unlock vault
    let client = PassClient::new(auth.access_token);
    let (vault, _key, _version) = client.unlock(&password).await?;

    // Search for matching items (case-insensitive)
    let query_lower = query.to_lowercase();
    let matches: Vec<_> = vault
        .items
        .iter()
        .filter(|item| item.deleted_at().is_none())
        .filter(|item| {
            let name_match = item.name().to_lowercase().contains(&query_lower);
            let username_match = match item {
                VaultItem::Password(p) => p.username.to_lowercase().contains(&query_lower),
                _ => false,
            };
            let url_match = match item {
                VaultItem::Password(p) => p
                    .urls
                    .iter()
                    .any(|u| u.to_lowercase().contains(&query_lower)),
                _ => false,
            };
            name_match || username_match || url_match
        })
        .collect();

    if matches.is_empty() {
        println!("{}", style(format!("No matches for '{}'", query)).red());
        return Ok(());
    }

    // If single match, use it directly
    let item = if matches.len() == 1 {
        matches[0]
    } else {
        // Multiple matches - prompt for selection
        println!("\n{}", style("Multiple matches:").bold());
        for (i, item) in matches.iter().enumerate() {
            let extra = match item {
                VaultItem::Password(p) if !p.username.is_empty() => {
                    format!(" ({})", style(&p.username).dim())
                }
                _ => String::new(),
            };
            println!("  {}. {} {}{}", i + 1, item.type_icon(), item.name(), extra);
        }
        println!();

        let selection: usize = dialoguer::Input::new()
            .with_prompt("Select")
            .validate_with(|input: &usize| {
                if *input >= 1 && *input <= matches.len() {
                    Ok(())
                } else {
                    Err(format!("Enter 1-{}", matches.len()))
                }
            })
            .interact()?;

        matches[selection - 1]
    };

    // Handle based on item type and flags
    match item {
        VaultItem::Password(p) => {
            if totp {
                if let Some(ref _totp_config) = p.totp {
                    // TODO: Implement TOTP generation
                    println!("{}", style("TOTP not yet implemented").yellow());
                } else {
                    println!("{}", style("No TOTP configured for this item").red());
                }
            } else if username {
                if show {
                    println!("{}", p.username);
                } else {
                    copy_to_clipboard(&p.username)?;
                    println!(
                        "{} {} {}",
                        style("✓").green(),
                        style("Username copied:").bold(),
                        p.username
                    );
                }
            } else {
                if show {
                    println!("{}", p.password);
                } else {
                    copy_to_clipboard(&p.password)?;
                    println!(
                        "{} {}",
                        style("✓").green(),
                        style(format!("Password copied for {}", p.name)).bold()
                    );
                }
            }
        }
        VaultItem::Card(c) => {
            if show {
                println!("Number: {}", c.number);
                println!("Expiry: {}/{}", c.exp_month, c.exp_year);
                println!("CVV: {}", c.cvv);
            } else {
                copy_to_clipboard(&c.number)?;
                println!(
                    "{} {}",
                    style("✓").green(),
                    style(format!("Card number copied for {}", c.name)).bold()
                );
            }
        }
        VaultItem::Note(n) => {
            if show {
                println!("{}", n.content);
            } else {
                copy_to_clipboard(&n.content)?;
                println!(
                    "{} {}",
                    style("✓").green(),
                    style(format!("Note copied for {}", n.name)).bold()
                );
            }
        }
        VaultItem::BankAccount(b) => {
            if show {
                println!("Account: {}", b.account_number);
                if let Some(ref routing) = b.routing_number {
                    println!("Routing: {}", routing);
                }
            } else {
                copy_to_clipboard(&b.account_number)?;
                println!(
                    "{} {}",
                    style("✓").green(),
                    style(format!("Account number copied for {}", b.name)).bold()
                );
            }
        }
        _ => {
            println!(
                "{}",
                style(format!("Cannot copy {} items", item.type_label())).yellow()
            );
        }
    }

    Ok(())
}

fn copy_to_clipboard(text: &str) -> Result<()> {
    use arboard::Clipboard;

    let mut clipboard = Clipboard::new().map_err(|e| anyhow!("Failed to access clipboard: {}", e))?;
    clipboard
        .set_text(text)
        .map_err(|e| anyhow!("Failed to copy to clipboard: {}", e))?;
    Ok(())
}
