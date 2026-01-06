use anyhow::{anyhow, Result};
use arboard::Clipboard;
use crossterm::event::KeyCode;

use super::app::{App, AppMode};
use crate::pass::totp;
use crate::pass::types::VaultItem;

pub fn handle_key(app: &mut App, key: KeyCode) -> Result<bool> {
    // Clear expired status messages
    app.clear_status_if_expired();

    match &app.mode {
        AppMode::Search(_) => handle_search_key(app, key),
        AppMode::Normal => handle_normal_key(app, key),
    }
}

fn handle_search_key(app: &mut App, key: KeyCode) -> Result<bool> {
    match key {
        KeyCode::Esc => {
            app.exit_search();
        }
        KeyCode::Enter => {
            // Exit search mode but keep the filter
            app.mode = AppMode::Normal;
        }
        KeyCode::Backspace => {
            app.search_backspace();
        }
        KeyCode::Char(c) => {
            app.search_input(c);
        }
        KeyCode::Up | KeyCode::Down => {
            // Allow navigation in search mode
            if key == KeyCode::Up {
                app.select_prev();
            } else {
                app.select_next();
            }
        }
        _ => {}
    }
    Ok(false)
}

fn handle_normal_key(app: &mut App, key: KeyCode) -> Result<bool> {
    match key {
        // Quit
        KeyCode::Char('q') | KeyCode::Esc => {
            app.should_quit = true;
            return Ok(true);
        }

        // Navigation
        KeyCode::Up | KeyCode::Char('k') => {
            app.select_prev();
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.select_next();
        }
        KeyCode::Char('g') => {
            app.select_first();
        }
        KeyCode::Char('G') => {
            app.select_last();
        }

        // Filter
        KeyCode::Char('f') | KeyCode::Tab => {
            app.cycle_filter();
        }

        // Search
        KeyCode::Char('/') => {
            app.enter_search();
        }

        // Copy password
        KeyCode::Char('c') => {
            if let Some(item) = app.selected_item() {
                let text = match item {
                    VaultItem::Password(p) => Some(p.password.clone()),
                    VaultItem::Card(c) => Some(c.number.clone()),
                    VaultItem::Note(n) => Some(n.content.clone()),
                    VaultItem::BankAccount(b) => Some(b.account_number.clone()),
                    _ => None,
                };

                if let Some(text) = text {
                    copy_to_clipboard(&text)?;
                    let label = match item {
                        VaultItem::Password(_) => "Password",
                        VaultItem::Card(_) => "Card number",
                        VaultItem::Note(_) => "Note",
                        VaultItem::BankAccount(_) => "Account number",
                        _ => "Content",
                    };
                    app.set_success(&format!("{} copied to clipboard", label));
                }
            }
        }

        // Copy username
        KeyCode::Char('u') => {
            if let Some(VaultItem::Password(p)) = app.selected_item() {
                if !p.username.is_empty() {
                    copy_to_clipboard(&p.username)?;
                    app.set_success("Username copied to clipboard");
                } else {
                    app.set_error("No username for this item");
                }
            } else {
                app.set_error("Only password items have usernames");
            }
        }

        // Copy TOTP
        KeyCode::Char('t') => {
            if let Some(VaultItem::Password(p)) = app.selected_item() {
                if let Some(ref totp_config) = p.totp {
                    match totp::generate(totp_config) {
                        Ok(code) => {
                            copy_to_clipboard(&code)?;
                            let remaining = totp::seconds_remaining(totp_config.period);
                            app.set_success(&format!("TOTP {} copied ({}s remaining)", code, remaining));
                        }
                        Err(e) => {
                            app.set_error(&format!("TOTP error: {}", e));
                        }
                    }
                } else {
                    app.set_error("No TOTP configured for this item");
                }
            } else {
                app.set_error("Only password items have TOTP");
            }
        }

        _ => {}
    }

    Ok(false)
}

fn copy_to_clipboard(text: &str) -> Result<()> {
    let mut clipboard =
        Clipboard::new().map_err(|e| anyhow!("Failed to access clipboard: {}", e))?;
    clipboard
        .set_text(text)
        .map_err(|e| anyhow!("Failed to copy to clipboard: {}", e))?;
    Ok(())
}
