use anyhow::{bail, Context, Result};
use std::process::Command;

/// Check if tmux is installed and meets minimum version
pub fn check_tmux() -> Result<()> {
    let output = Command::new("tmux")
        .arg("-V")
        .output()
        .context("tmux is required for groo dev. Install with: brew install tmux")?;

    if !output.status.success() {
        bail!("tmux is required for groo dev. Install with: brew install tmux");
    }

    let version_str = String::from_utf8_lossy(&output.stdout);
    let version = version_str
        .trim()
        .strip_prefix("tmux ")
        .unwrap_or("")
        .split('.')
        .next()
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(0);

    if version < 3 {
        bail!(
            "tmux >= 3.0 required (found {}). Update with: brew upgrade tmux",
            version_str.trim()
        );
    }

    Ok(())
}

/// Check if a tmux session exists
pub fn session_exists(session: &str) -> bool {
    Command::new("tmux")
        .args(["has-session", "-t", session])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Create a new tmux session (detached) with the first window
pub fn new_session(session: &str, window_name: &str, command: &str) -> Result<()> {
    let status = Command::new("tmux")
        .args([
            "new-session", "-d",
            "-s", session,
            "-n", window_name,
            command,
        ])
        .status()
        .context("Failed to create tmux session")?;

    if !status.success() {
        bail!("Failed to create tmux session '{}'", session);
    }
    Ok(())
}

/// Add a new window to an existing session
pub fn new_window(session: &str, window_name: &str, command: &str) -> Result<()> {
    let status = Command::new("tmux")
        .args([
            "new-window",
            "-t", session,
            "-n", window_name,
            command,
        ])
        .status()
        .context("Failed to create tmux window")?;

    if !status.success() {
        bail!("Failed to create window '{}' in session '{}'", window_name, session);
    }
    Ok(())
}

/// Set a session-scoped tmux option
pub fn set_option(session: &str, option: &str, value: &str) -> Result<()> {
    Command::new("tmux")
        .args(["set-option", "-t", session, option, value])
        .status()
        .context(format!("Failed to set tmux option {} = {}", option, value))?;
    Ok(())
}

/// Set a global tmux option
#[allow(dead_code)]
pub fn set_global_option(option: &str, value: &str) -> Result<()> {
    Command::new("tmux")
        .args(["set-option", "-g", option, value])
        .status()
        .context(format!("Failed to set global tmux option {} = {}", option, value))?;
    Ok(())
}

/// Set a global tmux window option
pub fn set_global_window_option(option: &str, value: &str) -> Result<()> {
    Command::new("tmux")
        .args(["set-window-option", "-g", option, value])
        .status()
        .context(format!("Failed to set global window option {} = {}", option, value))?;
    Ok(())
}

/// Bind a key (prefix key + key → tmux command)
pub fn bind_key(key: &str, args: &[&str]) -> Result<()> {
    let mut cmd_args = vec!["bind-key", key];
    cmd_args.extend_from_slice(args);
    Command::new("tmux")
        .args(&cmd_args)
        .status()
        .context(format!("Failed to bind key {}", key))?;
    Ok(())
}

/// Attach to a session (blocks until detach)
pub fn attach_session(session: &str) -> Result<()> {
    let status = Command::new("tmux")
        .args(["attach-session", "-t", session])
        .status()
        .context("Failed to attach to tmux session")?;

    if !status.success() {
        bail!("tmux attach exited with error");
    }
    Ok(())
}

/// Switch client to a session (when already inside tmux)
pub fn switch_client(session: &str) -> Result<()> {
    let status = Command::new("tmux")
        .args(["switch-client", "-t", session])
        .status()
        .context("Failed to switch tmux client")?;

    if !status.success() {
        bail!("tmux switch-client exited with error");
    }
    Ok(())
}

/// Kill a tmux session
pub fn kill_session(session: &str) -> Result<()> {
    Command::new("tmux")
        .args(["kill-session", "-t", session])
        .status()
        .context("Failed to kill tmux session")?;
    Ok(())
}

/// Select (focus) a specific window
pub fn select_window(session: &str, window: &str) -> Result<()> {
    let target = format!("{}:{}", session, window);
    Command::new("tmux")
        .args(["select-window", "-t", &target])
        .status()
        .context("Failed to select tmux window")?;
    Ok(())
}

/// Check if we're currently inside a tmux session
pub fn is_inside_tmux() -> bool {
    std::env::var("TMUX").is_ok()
}
