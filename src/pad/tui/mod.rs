mod app;
mod events;
mod ui;

pub use app::{App, DecryptedFile, DecryptedItem};

use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::prelude::*;
use std::io;
use std::time::Duration;

use crate::pad::client::PadClient;
use crate::pad::crypto::decrypt;
use crate::pad::types::UserState;

/// Run the TUI for viewing and managing pad items
pub async fn run(token: String, password: &str) -> Result<()> {
    // Connect and get initial state
    let client = PadClient::new(token.clone());
    let (state, key) = client.connect_and_sync(password).await?;

    // Decrypt all items
    let items = decrypt_items(&state, &key)?;

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app state
    let mut app = App::new(items, key, token);

    // Main loop
    let result = run_app(&mut terminal, &mut app, &client).await;

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    result
}

async fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
    client: &PadClient,
) -> Result<()> {
    loop {
        terminal.draw(|f| ui::render(f, app))?;

        // Poll for events with timeout for async operations
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match events::handle_key(app, key.code, client).await {
                        Ok(should_quit) if should_quit => break,
                        Err(e) => app.set_error(&e.to_string()),
                        _ => {}
                    }
                }
            }
        }

        if app.should_quit {
            break;
        }
    }

    Ok(())
}

fn decrypt_items(state: &UserState, key: &[u8; 32]) -> Result<Vec<DecryptedItem>> {
    let mut items = Vec::new();

    for item in &state.list {
        let text = decrypt(&item.encrypted_text, key).unwrap_or_else(|_| "[decryption failed]".to_string());

        let files: Vec<DecryptedFile> = item
            .files
            .iter()
            .map(|f| {
                let name = decrypt(&f.encrypted_name, key).unwrap_or_else(|_| "unknown".to_string());
                let mime_type = decrypt(&f.encrypted_type, key).unwrap_or_else(|_| "application/octet-stream".to_string());
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

    Ok(items)
}
