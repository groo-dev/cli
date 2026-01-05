use anyhow::Result;
use arboard::Clipboard;
use crossterm::event::KeyCode;
use std::path::PathBuf;

use super::app::{App, AppMode, DecryptedFile, DecryptedItem, StatusType};
use crate::pad::client::PadClient;
use crate::pad::crypto::decrypt;
use crate::pad::types::UserState;

/// Handle a key press event. Returns true if the app should quit.
pub async fn handle_key(app: &mut App, key: KeyCode, client: &PadClient) -> Result<bool> {
    // Clear expired status messages
    app.clear_status_if_expired();

    // Dispatch based on current mode
    match &app.mode {
        AppMode::DirectoryPicker(_) => handle_picker_key(app, key, client).await,
        AppMode::ConfirmDelete(_) => handle_confirm_delete_key(app, key),
        AppMode::Normal => handle_normal_key(app, key, client).await,
    }
}

async fn handle_normal_key(app: &mut App, key: KeyCode, client: &PadClient) -> Result<bool> {
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
            // Check if there are files to download first
            if let Some(item) = app.selected_item() {
                if item.files.is_empty() {
                    app.set_status("No files to download", StatusType::Info);
                } else {
                    app.start_dir_picker();
                }
            } else {
                app.set_error("No item selected");
            }
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

fn handle_confirm_delete_key(app: &mut App, key: KeyCode) -> Result<bool> {
    // Extract item_id before modifying app
    let item_id = if let AppMode::ConfirmDelete(id) = &app.mode {
        id.clone()
    } else {
        return Ok(false);
    };

    match key {
        KeyCode::Char('y') | KeyCode::Char('Y') => {
            // TODO: Send delete message to server when persistent WS is implemented
            app.remove_item(&item_id);
            app.mode = AppMode::Normal;
            app.set_success("Item deleted");
        }
        _ => {
            app.cancel_mode();
        }
    }

    Ok(false)
}

async fn handle_picker_key(app: &mut App, key: KeyCode, client: &PadClient) -> Result<bool> {
    // We need to handle the picker state carefully due to borrow checker
    match key {
        KeyCode::Esc => {
            app.cancel_mode();
        }

        KeyCode::Up | KeyCode::Char('k') => {
            if let AppMode::DirectoryPicker(ref mut picker) = app.mode {
                picker.select_prev();
            }
        }

        KeyCode::Down | KeyCode::Char('j') => {
            if let AppMode::DirectoryPicker(ref mut picker) = app.mode {
                picker.select_next();
            }
        }

        KeyCode::Enter => {
            // Navigate into selected directory
            if let AppMode::DirectoryPicker(ref mut picker) = app.mode {
                if let Err(e) = picker.navigate_into() {
                    app.set_error(&format!("Failed to open directory: {}", e));
                }
            }
        }

        KeyCode::Char(' ') => {
            // Select current directory and download
            let download_dir = if let AppMode::DirectoryPicker(ref picker) = app.mode {
                Some(picker.current_dir.clone())
            } else {
                None
            };

            if let Some(dir) = download_dir {
                app.mode = AppMode::Normal;
                do_download(app, client, &dir).await?;
            }
        }

        KeyCode::Char('~') => {
            // Go to home directory
            if let AppMode::DirectoryPicker(ref mut picker) = app.mode {
                if let Some(home) = dirs::home_dir() {
                    picker.current_dir = home;
                    if let Err(e) = picker.refresh() {
                        app.set_error(&format!("Failed to open home directory: {}", e));
                    }
                }
            }
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

async fn do_download(app: &mut App, client: &PadClient, download_dir: &PathBuf) -> Result<()> {
    let Some(item) = app.selected_item().cloned() else {
        app.set_error("No item selected");
        return Ok(());
    };

    if item.files.is_empty() {
        app.set_status("No files to download", StatusType::Info);
        return Ok(());
    }

    // Ensure directory exists
    if !download_dir.exists() {
        if let Err(e) = std::fs::create_dir_all(download_dir) {
            app.set_error(&format!("Failed to create directory: {}", e));
            return Ok(());
        }
    }

    // Download each file
    let mut success_count = 0;
    for file in &item.files {
        match client.download_file(&file.r2_key, &app.key).await {
            Ok(data) => {
                let file_path = download_dir.join(&file.name);
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
        download_dir.display()
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
