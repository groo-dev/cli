use anyhow::Result;
use console::{style, Style, Term};
use dialoguer::{theme::ColorfulTheme, MultiSelect};
use tokio::sync::broadcast;

use crate::config::get_service_log_file;
use crate::discovery::{discover_services, find_git_root, get_project_name, Service};
use crate::runner::{get_color_for_index, spawn_service, wait_for_processes, ProcessHandle};
use crate::state::{is_port_in_use, State};

fn create_theme() -> ColorfulTheme {
    ColorfulTheme {
        defaults_style: Style::new().dim(),
        prompt_style: Style::new().bold(),
        prompt_prefix: style("?".to_string()).green().bold(),
        success_prefix: style("✓".to_string()).green().bold(),
        error_prefix: style("✗".to_string()).red().bold(),
        checked_item_prefix: style("  ◉".to_string()).green(),
        unchecked_item_prefix: style("  ○".to_string()).dim(),
        active_item_style: Style::new().cyan().bold(),
        inactive_item_style: Style::new().dim(),
        active_item_prefix: style("❯".to_string()).cyan().bold(),
        ..ColorfulTheme::default()
    }
}

pub async fn run() -> Result<()> {
    let git_root = find_git_root()?;
    let project_name = get_project_name(&git_root);
    let services = discover_services(&git_root)?;

    // Filter to only running services (port-based detection)
    let running_service_list: Vec<&Service> = services
        .iter()
        .filter(|s| s.port.map(is_port_in_use).unwrap_or(false))
        .collect();

    if running_service_list.is_empty() {
        println!(
            "{} No running services found. Use {} to start services.",
            style("!").yellow(),
            style("gr dev").cyan()
        );
        return Ok(());
    }

    // Find max name length for alignment
    let max_name_len = running_service_list.iter().map(|s| s.name.len()).max().unwrap_or(0);

    // Display running services for selection
    let items: Vec<String> = running_service_list
        .iter()
        .map(|s| {
            let port_str = s.port
                .map(|p| format!("{}", p))
                .unwrap_or_else(|| "-".to_string());
            format!(
                "{:<width$}  {}",
                s.name,
                style(port_str).dim(),
                width = max_name_len
            )
        })
        .collect();

    // All selected by default
    let defaults: Vec<bool> = vec![true; running_service_list.len()];

    let theme = create_theme();
    let selections = MultiSelect::with_theme(&theme)
        .with_prompt("Select services to restart")
        .items(&items)
        .defaults(&defaults)
        .interact_on(&Term::stderr())?;

    if selections.is_empty() {
        println!("{}", style("No services selected.").yellow());
        return Ok(());
    }

    let selected_services: Vec<_> = selections
        .iter()
        .map(|&i| running_service_list[i])
        .collect();

    // Stop selected services
    println!(
        "\n{} Stopping {} service(s)...\n",
        style("→").yellow().bold(),
        selected_services.len()
    );

    for service in &selected_services {
        if let Some(port) = service.port {
            if let Some(pid) = get_pid_by_port(port) {
                if kill_process(pid) {
                    println!(
                        "  {} Stopped {}",
                        style("✓").green(),
                        service.name
                    );
                } else {
                    println!(
                        "  {} Failed to stop {}",
                        style("✗").red(),
                        service.name
                    );
                }
            }
        }
    }

    // Clean state
    let mut state = State::load().unwrap_or_default();
    state.clean_stale_pids();
    state.save()?;

    // Brief pause to allow ports to be released
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    // Start selected services
    println!(
        "\n{} Starting {} service(s)...\n",
        style("→").green().bold(),
        selected_services.len()
    );

    // Set up shutdown signal
    let (shutdown_tx, _) = broadcast::channel::<()>(1);

    // Set up Ctrl+C handler
    let shutdown_tx_clone = shutdown_tx.clone();
    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.ok();
        println!("\n{} Shutting down...", style("→").yellow().bold());
        let _ = shutdown_tx_clone.send(());
    });

    // Reload state
    let mut state = State::load().unwrap_or_default();

    // Spawn all selected services
    let mut handles: Vec<ProcessHandle> = Vec::new();
    for (idx, service) in selected_services.iter().enumerate() {
        let color = get_color_for_index(idx);
        let log_file = get_service_log_file(&service.path);

        match spawn_service(
            &service.name,
            &service.path,
            &service.dev_command,
            color.clone(),
            log_file,
        )
        .await
        {
            Ok(handle) => {
                if let Some(pid) = handle.pid() {
                    state.add_service(
                        &project_name,
                        git_root.clone(),
                        &service.name,
                        pid,
                        service.port,
                    );
                }
                handles.push(handle);
            }
            Err(e) => {
                eprintln!(
                    "{} Failed to start {}: {}",
                    style("✗").red().bold(),
                    service.name,
                    e
                );
            }
        }
    }

    // Save state
    state.save()?;

    // Wait for all processes or shutdown
    let shutdown_rx = shutdown_tx.subscribe();
    wait_for_processes(handles, shutdown_rx).await;

    // Clean up state on exit
    let mut state = State::load().unwrap_or_default();
    for service in &selected_services {
        state.remove_service(&project_name, &service.name);
    }
    state.save()?;

    Ok(())
}

/// Get PID of process listening on a port using lsof
#[cfg(unix)]
fn get_pid_by_port(port: u16) -> Option<u32> {
    use std::process::Command;
    let output = Command::new("lsof")
        .args(["-ti", &format!(":{}", port)])
        .output()
        .ok()?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        // lsof can return multiple PIDs, take the first one
        stdout.lines().next()?.trim().parse().ok()
    } else {
        None
    }
}

#[cfg(not(unix))]
fn get_pid_by_port(port: u16) -> Option<u32> {
    use std::process::Command;
    let output = Command::new("netstat")
        .args(["-ano"])
        .output()
        .ok()?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            if line.contains(&format!(":{}", port)) && line.contains("LISTENING") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if let Some(pid_str) = parts.last() {
                    return pid_str.parse().ok();
                }
            }
        }
    }
    None
}

#[cfg(unix)]
fn kill_process(pid: u32) -> bool {
    use std::process::Command;
    Command::new("kill")
        .args(["-15", &pid.to_string()])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn kill_process(pid: u32) -> bool {
    use std::process::Command;
    Command::new("taskkill")
        .args(["/F", "/PID", &pid.to_string()])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}
