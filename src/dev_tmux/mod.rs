pub mod tmux;

use anyhow::Result;
use console::style;
use std::path::PathBuf;

use crate::discovery::Service;

/// Sanitize a name for use in tmux targets.
/// Dots are pane separators, colons are window separators in tmux's target syntax.
fn sanitize_tmux_name(name: &str) -> String {
    name.replace(['.', ':'], "-")
}

/// Build the shell command for a service window.
/// Falls back to a shell prompt if the command exits, so the window stays alive.
fn service_cmd(path: &std::path::Path, dev_command: &str) -> String {
    format!("cd '{}' && {}; exec $SHELL", path.display(), dev_command)
}

/// Run dev servers in a tmux session — one window per service
pub fn run(
    _project_name: String,
    _git_root: PathBuf,
    services: Vec<Service>,
) -> Result<()> {
    tmux::check_tmux()?;

    let session = format!("groo-{}", sanitize_tmux_name(&_project_name));

    // Handle existing session
    if tmux::session_exists(&session)
        && !handle_existing_session(&session)?
    {
        return Ok(());
    }

    // Create session with first service as window 0
    let first = &services[0];
    let first_window = sanitize_tmux_name(&first.name);
    let first_cmd = service_cmd(&first.path, &first.dev_command);
    tmux::new_session(&session, &first_window, &first_cmd)?;

    // Create windows for remaining services
    for service in &services[1..] {
        let window_name = sanitize_tmux_name(&service.name);
        let cmd = service_cmd(&service.path, &service.dev_command);
        tmux::new_window(&session, &window_name, &cmd)?;
    }

    // Configure session
    configure_session(&session, services.len())?;

    // Focus first window
    tmux::select_window(&session, &first_window)?;

    // Attach (or switch if already inside tmux)
    println!(
        "{} Starting tmux session '{}'...",
        style("→").green().bold(),
        session
    );

    if tmux::is_inside_tmux() {
        tmux::switch_client(&session)?;
    } else {
        tmux::attach_session(&session)?;
    }

    Ok(())
}

/// Handle an existing tmux session: prompt to attach or replace
fn handle_existing_session(session: &str) -> Result<bool> {
    use dialoguer::Confirm;

    let attach = Confirm::new()
        .with_prompt(format!(
            "Session '{}' is already running. Attach to it?",
            session
        ))
        .default(true)
        .interact()?;

    if attach {
        if tmux::is_inside_tmux() {
            tmux::switch_client(session)?;
        } else {
            tmux::attach_session(session)?;
        }
        return Ok(false);
    }

    let replace = Confirm::new()
        .with_prompt("Replace it?")
        .default(false)
        .interact()?;

    if replace {
        tmux::kill_session(session)?;
        Ok(true)
    } else {
        Ok(false)
    }
}

/// Configure all tmux session settings
fn configure_session(session: &str, service_count: usize) -> Result<()> {
    // -- Behavior --
    tmux::set_option(session, "mouse", "on")?;
    tmux::set_option(session, "history-limit", "50000")?;
    tmux::set_option(session, "escape-time", "0")?;
    tmux::set_option(session, "renumber-windows", "on")?;
    tmux::set_option(session, "default-terminal", "screen-256color")?;
    tmux::set_option(session, "focus-events", "on")?;

    // Keep service names — don't let tmux rename to the running process
    tmux::set_option(session, "allow-rename", "off")?;
    tmux::set_option(session, "automatic-rename", "off")?;

    // Highlight windows with new output
    tmux::set_option(session, "monitor-activity", "on")?;
    tmux::set_option(session, "visual-activity", "off")?; // highlight only, no message

    // -- Status bar layout --
    tmux::set_option(session, "status-position", "bottom")?;
    tmux::set_option(session, "status-justify", "left")?;
    tmux::set_option(session, "status-interval", "1")?;

    // Status bar background
    tmux::set_option(session, "status-style", "bg=#1e1e2e,fg=#6c7086")?;

    // Left: session name
    tmux::set_option(session, "status-left", " #[fg=#cdd6f4,bold]#S #[fg=#45475a]│ ")?;
    tmux::set_option(session, "status-left-length", "30")?;

    // Right: service count
    let right = format!("#[fg=#6c7086] {} services ", service_count);
    tmux::set_option(session, "status-right", &right)?;
    tmux::set_option(session, "status-right-length", "20")?;

    // Window tabs: inactive
    tmux::set_option(
        session,
        "window-status-format",
        " #[fg=#6c7086]#W ",
    )?;

    // Window tabs: active (colored pill)
    tmux::set_option(
        session,
        "window-status-current-format",
        "#[fg=#1e1e2e,bg=#89b4fa,bold] #W #[default]",
    )?;

    // Window tabs: activity (yellow text)
    tmux::set_option(session, "window-status-activity-style", "fg=#f9e2af,bg=default,none")?;

    // Separator between windows
    tmux::set_option(session, "window-status-separator", "")?;

    // Message and command prompt styling
    tmux::set_option(session, "message-style", "bg=#313244,fg=#cdd6f4")?;

    Ok(())
}
