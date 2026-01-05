use anyhow::Result;
use arboard::Clipboard;
use crossterm::event::KeyCode;
use dialoguer::Input;
use std::path::PathBuf;

use super::app::{App, ConfirmAction, DecryptedFile, DecryptedItem, StatusType};
use crate::pad::client::PadClient;
use crate::pad::crypto::decrypt;
use crate::pad::types::UserState;

/// Handle a key press event. Returns true if the app should quit.
pub async fn handle_key(app: &mut App, key: KeyCode, client: &PadClient) -> Result<bool> {
    // Clear expired status messages
    app.clear_status_if_expired();

    // Handle confirmation mode
    if let ConfirmAction::Delete(item_id) = &app.confirm_action {
        let item_id = item_id.clone();
        match key {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                // TODO: Send delete message to server when persistent WS is implemented
                app.remove_item(&item_id);
                app.confirm_action = ConfirmAction::None;
                app.set_success("Item deleted");
            }
            _ => {
                app.cancel_confirm();
            }
        }
        return Ok(false);
    }

    // Normal mode
    match key {
        KeyCode::Char('q') | KeyCode::Esc => {
            app.should_quit = true;
            return Ok(true);
        }

        KeyCode::Up | KeyCode::Char('k') => {
            app.select_prev();
        }

        KeyCode::Down | KeyCode::Char('j') => {
            app.select_next();
        }

        KeyCode::Char('c') => {
            copy_to_clipboard(app)?;
        }

        KeyCode::Char('d') => {
            download_files(app, client).await?;
        }

        KeyCode::Char('x') | KeyCode::Delete => {
            app.start_delete_confirm();
        }

        KeyCode::Char('r') => {
            refresh_items(app, client).await?;
        }

        _ => {}
    }

    Ok(false)
}

fn copy_to_clipboard(app: &mut App) -> Result<()> {
    let Some(item) = app.selected_item() else {
        app.set_error("No item selected");
        return Ok(());
    };

    match Clipboard::new() {
        Ok(mut clipboard) => {
            if let Err(e) = clipboard.set_text(&item.text) {
                app.set_error(&format!("Failed to copy: {}", e));
            } else {
                app.set_success("Copied to clipboard");
            }
        }
        Err(e) => {
            app.set_error(&format!("Clipboard not available: {}", e));
        }
    }

    Ok(())
}

async fn download_files(app: &mut App, client: &PadClient) -> Result<()> {
    let Some(item) = app.selected_item().cloned() else {
        app.set_error("No item selected");
        return Ok(());
    };

    if item.files.is_empty() {
        app.set_status("No files to download", StatusType::Info);
        return Ok(());
    }

    // Get download directory from user
    // Note: This will temporarily exit raw mode for the prompt
    crossterm::terminal::disable_raw_mode()?;

    let default_dir = dirs::download_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .to_string_lossy()
        .to_string();

    let download_dir: String = Input::new()
        .with_prompt("Download to directory")
        .default(default_dir)
        .interact_text()?;

    crossterm::terminal::enable_raw_mode()?;

    let download_path = PathBuf::from(&download_dir);
    if !download_path.exists() {
        std::fs::create_dir_all(&download_path)?;
    }

    // Download each file
    let mut success_count = 0;
    for file in &item.files {
        match client.download_file(&file.r2_key, &app.key).await {
            Ok(data) => {
                let file_path = download_path.join(&file.name);
                if let Err(e) = std::fs::write(&file_path, &data) {
                    app.set_error(&format!("Failed to save {}: {}", file.name, e));
                    return Ok(());
                }
                success_count += 1;
            }
            Err(e) => {
                app.set_error(&format!("Failed to download {}: {}", file.name, e));
                return Ok(());
            }
        }
    }

    app.set_success(&format!(
        "Downloaded {} file{} to {}",
        success_count,
        if success_count == 1 { "" } else { "s" },
        download_dir
    ));

    Ok(())
}

async fn refresh_items(app: &mut App, client: &PadClient) -> Result<()> {
    app.set_status("Refreshing...", StatusType::Info);

    match client.fetch_state().await {
        Ok(state) => {
            let items = decrypt_items(&state, &app.key);
            let count = items.len();
            app.update_items(items);
            app.set_success(&format!("Refreshed ({} items)", count));
        }
        Err(e) => {
            app.set_error(&format!("Refresh failed: {}", e));
        }
    }

    Ok(())
}

fn decrypt_items(state: &UserState, key: &[u8; 32]) -> Vec<DecryptedItem> {
    let mut items = Vec::new();

    for item in &state.list {
        let text = decrypt(&item.encrypted_text, key)
            .unwrap_or_else(|_| "[decryption failed]".to_string());

        let files: Vec<DecryptedFile> = item
            .files
            .iter()
            .map(|f| {
                let name = decrypt(&f.encrypted_name, key)
                    .unwrap_or_else(|_| "unknown".to_string());
                let mime_type = decrypt(&f.encrypted_type, key)
                    .unwrap_or_else(|_| "application/octet-stream".to_string());
                DecryptedFile {
                    id: f.id.clone(),
                    name,
                    mime_type,
                    size: f.size,
                    r2_key: f.r2_key.clone(),
                }
            })
            .collect();

        items.push(DecryptedItem {
            id: item.id.clone(),
            text,
            files,
            created_at: item.created_at,
        });
    }

    items
}
