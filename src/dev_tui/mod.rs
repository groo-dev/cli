mod app;
mod events;
mod logs;
mod stats;
mod ui;

use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io::stdout;
use std::path::PathBuf;
use std::time::Duration;
use tokio::sync::{broadcast, mpsc};

pub use app::App;
pub use logs::LogMessage;

use crate::discovery::Service;

/// Run the dev TUI with the given services
pub async fn run(
    project_name: String,
    git_root: PathBuf,
    services: Vec<Service>,
) -> Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app state
    let (shutdown_tx, _) = broadcast::channel::<()>(1);
    let (log_tx, log_rx) = mpsc::unbounded_channel::<LogMessage>();

    let mut app = App::new(
        project_name,
        git_root,
        services,
        shutdown_tx.clone(),
        log_tx,
    );

    // Run the app
    let result = run_app(&mut terminal, &mut app, log_rx).await;

    // Show shutdown message
    let running_count = app.running_count();
    if running_count > 0 {
        terminal.draw(|f| ui::render_shutdown(f, running_count))?;
    }

    // Stop all services before restoring terminal
    app.shutdown().await;

    // Signal shutdown (for any listeners)
    let _ = shutdown_tx.send(());

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
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    app: &mut App,
    mut log_rx: mpsc::UnboundedReceiver<LogMessage>,
) -> Result<()> {
    // Start all services initially
    app.start_all_services().await?;

    loop {
        // Draw UI
        terminal.draw(|f| ui::render(f, app))?;

        // Collect logs from channel (non-blocking)
        while let Ok(msg) = log_rx.try_recv() {
            app.logs.push(msg);
            if app.follow_mode {
                app.scroll_to_bottom();
            }
        }

        // Update stats periodically
        app.maybe_update_stats();

        // Check process statuses
        app.check_processes().await;

        // Poll for input events
        if event::poll(Duration::from_millis(50))?
            && let Event::Key(key) = event::read()?
            && events::handle_key(app, key).await?
        {
            break; // Quit requested
        }

        if app.should_quit {
            break;
        }
    }

    Ok(())
}
