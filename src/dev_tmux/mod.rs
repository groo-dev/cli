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
    let first_cmd = format!("cd {} && {}", first.path.display(), first.dev_command);
    tmux::new_session(&session, &first_window, &first_cmd)?;

    // Create windows for remaining services
    for service in &services[1..] {
        let window_name = sanitize_tmux_name(&service.name);
        let cmd = format!("cd {} && {}", service.path.display(), service.dev_command);
        tmux::new_window(&session, &window_name, &cmd)?;
    }

    // Style the status bar
    configure_status_bar(&session, services.len())?;

    // Set remain-on-exit so crash output stays visible
    tmux::set_option(&session, "remain-on-exit", "on")?;

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

/// Configure the tmux status bar with groo styling
fn configure_status_bar(session: &str, service_count: usize) -> Result<()> {
    tmux::set_option(session, "status-style", "bg=colour236,fg=colour248")?;
    tmux::set_option(session, "window-status-current-style", "bg=colour239,fg=colour223,bold")?;
    tmux::set_option(session, "window-status-style", "fg=colour246")?;

    let right = format!(" {} services ", service_count);
    tmux::set_option(session, "status-right", &right)?;
    tmux::set_option(session, "status-left", " [#S] ")?;
    tmux::set_option(session, "status-left-length", "20")?;
    tmux::set_option(session, "status-right-length", "20")?;

    Ok(())
}
