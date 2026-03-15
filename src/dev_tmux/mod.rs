mod aggregate;
pub mod tmux;

use anyhow::Result;
use console::style;
use std::path::PathBuf;

use crate::config;
use crate::discovery::Service;
use crate::state::State;

/// Sanitize a name for use in tmux targets.
/// Dots are pane separators, colons are window separators in tmux's target syntax.
fn sanitize_tmux_name(name: &str) -> String {
    name.replace(['.', ':'], "-")
}

/// Run the dev TUI via tmux session
pub async fn run(
    project_name: String,
    git_root: PathBuf,
    services: Vec<Service>,
) -> Result<()> {
    tmux::check_tmux()?;

    let session = format!("groo-{}", sanitize_tmux_name(&project_name));

    // Handle existing session
    if tmux::session_exists(&session)
        && !handle_existing_session(&session)? {
            return Ok(());
        }

    // Ensure log directory exists
    config::ensure_project_logs_dir(&project_name)?;

    // Truncate existing log files for fresh session
    let logs_dir = config::get_project_logs_dir(&project_name);
    for service in &services {
        let log_file = logs_dir.join(format!("{}.log", service.name));
        if log_file.exists() {
            std::fs::write(&log_file, "")?;
        }
    }

    // Build the aggregate command for window 0
    let groo_bin = std::env::current_exe()?;
    let aggregate_cmd = format!(
        "{} dev --aggregate --project {}",
        groo_bin.display(),
        project_name
    );

    // Create session with "all" window (window 0)
    tmux::new_session(&session, "all", &aggregate_cmd)?;

    // Create one window per service
    for service in &services {
        let window_name = sanitize_tmux_name(&service.name);
        let cmd = format!(
            "cd {} && {}",
            service.path.display(),
            service.dev_command
        );
        tmux::new_window(&session, &window_name, &cmd)?;

        // Set up log capture via pipe-pane (ANSI-stripped)
        let log_file = config::get_service_log_file(&project_name, &service.name);
        tmux::pipe_pane(&session, &window_name, &log_file.display().to_string())?;
    }

    // Style the status bar
    configure_status_bar(&session, services.len())?;

    // Set remain-on-exit so crash output stays visible
    tmux::set_option(&session, "remain-on-exit", "on")?;

    // Save state
    let mut state = State::load().unwrap_or_default();
    for service in &services {
        let window_name = sanitize_tmux_name(&service.name);
        let pid = tmux::get_pane_pid(&session, &window_name).unwrap_or(0);
        state.add_service(
            &project_name,
            git_root.clone(),
            &service.name,
            pid,
            service.port,
        );
    }
    state.set_tmux_session(&project_name, &session);
    state.save()?;

    // Focus window 0 ("all")
    tmux::select_window(&session, "all")?;

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

/// Run the aggregate log tailer (called as hidden subcommand)
pub async fn run_aggregate(project: &str) -> Result<()> {
    aggregate::run(project).await
}
