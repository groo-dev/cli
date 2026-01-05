use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::app::{App, AppMode, PaneFocus};

/// Handle a key event. Returns true if the app should quit.
pub async fn handle_key(app: &mut App, key: KeyEvent) -> Result<bool> {
    // Handle Ctrl+C globally
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        app.should_quit = true;
        return Ok(true);
    }

    match app.mode {
        AppMode::Normal => handle_normal_mode(app, key).await,
        AppMode::Search => handle_search_mode(app, key),
        AppMode::Help => handle_help_mode(app, key),
        AppMode::Visual => handle_visual_mode(app, key),
    }
}

async fn handle_normal_mode(app: &mut App, key: KeyEvent) -> Result<bool> {
    // Global keys (work in both panes)
    match key.code {
        KeyCode::Char('q') => {
            app.should_quit = true;
            return Ok(true);
        }
        KeyCode::Tab => {
            app.toggle_focus();
            return Ok(false);
        }
        KeyCode::Char('?') => {
            app.toggle_help();
            return Ok(false);
        }
        KeyCode::Char('/') => {
            app.enter_search();
            return Ok(false);
        }
        _ => {}
    }

    // Focus-specific keys
    match app.focus {
        PaneFocus::Services => handle_services_keys(app, key).await,
        PaneFocus::Logs => handle_logs_keys(app, key).await,
    }
}

async fn handle_services_keys(app: &mut App, key: KeyEvent) -> Result<bool> {
    match key.code {
        // Navigation
        KeyCode::Up | KeyCode::Char('k') => {
            app.cursor_up();
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.cursor_down();
        }

        // Selection
        KeyCode::Char(' ') => {
            app.toggle_selection();
        }
        KeyCode::Char('a') => {
            app.select_all();
        }
        KeyCode::Char('n') => {
            app.select_none();
        }

        // Service controls
        KeyCode::Char('r') => {
            app.restart_current().await?;
        }
        KeyCode::Char('R') => {
            app.restart_all().await?;
        }
        KeyCode::Char('s') => {
            app.stop_current().await?;
        }
        KeyCode::Char('S') => {
            app.stop_all().await?;
        }

        // Open in browser
        KeyCode::Char('o') => {
            app.open_current_in_browser();
        }

        _ => {}
    }

    Ok(false)
}

async fn handle_logs_keys(app: &mut App, key: KeyEvent) -> Result<bool> {
    match key.code {
        // Navigation
        KeyCode::Up | KeyCode::Char('k') => {
            app.log_cursor_up();
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.log_cursor_down(30); // Approximate visible height
        }

        // Jump to top/bottom
        KeyCode::Char('g') => {
            app.log_cursor_top();
        }
        KeyCode::Char('G') => {
            app.log_cursor_bottom();
        }

        // Page scroll
        KeyCode::PageUp => {
            app.scroll_up(10);
        }
        KeyCode::PageDown => {
            app.scroll_down(10, 50);
        }

        // Visual mode (log selection)
        KeyCode::Char('v') => {
            app.enter_visual_mode();
        }

        // Copy current line
        KeyCode::Char('y') => {
            app.copy_current_line()?;
        }

        _ => {}
    }

    Ok(false)
}

fn handle_search_mode(app: &mut App, key: KeyEvent) -> Result<bool> {
    match key.code {
        // Exit search
        KeyCode::Esc => {
            app.exit_search();
        }
        KeyCode::Enter => {
            app.mode = AppMode::Normal;
            // Keep search results highlighted
        }

        // Navigate matches
        KeyCode::Char('n') if key.modifiers.is_empty() => {
            app.next_match();
        }
        KeyCode::Char('N') | KeyCode::Char('p') => {
            app.prev_match();
        }

        // Type search query
        KeyCode::Char(c) => {
            app.search.query.push(c);
            app.update_search();
        }
        KeyCode::Backspace => {
            app.search.query.pop();
            app.update_search();
        }

        _ => {}
    }

    Ok(false)
}

fn handle_help_mode(app: &mut App, _key: KeyEvent) -> Result<bool> {
    // Any key closes help
    app.mode = AppMode::Normal;
    Ok(false)
}

fn handle_visual_mode(app: &mut App, key: KeyEvent) -> Result<bool> {
    match key.code {
        // Exit visual mode
        KeyCode::Esc => {
            app.exit_visual_mode();
        }

        // Move selection
        KeyCode::Up | KeyCode::Char('k') => {
            app.visual_move_up();
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.visual_move_down(30); // Approximate visible height
        }

        // Copy selection
        KeyCode::Char('y') => {
            app.copy_selection()?;
            app.exit_visual_mode();
        }

        // Scroll without changing selection
        KeyCode::PageUp => {
            app.scroll_up(10);
        }
        KeyCode::PageDown => {
            app.scroll_down(10, 50);
        }

        _ => {}
    }

    Ok(false)
}
