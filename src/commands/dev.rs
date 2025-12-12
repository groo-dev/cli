use anyhow::Result;
use console::{style, Style, Term};
use dialoguer::{theme::ColorfulTheme, MultiSelect};
use tokio::sync::broadcast;

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

    if services.is_empty() {
        println!("{}", style("No services with dev scripts found.").yellow());
        return Ok(());
    }

    // Load state
    let mut state = State::load().unwrap_or_default();
    state.clean_stale_pids();
    state.save()?;

    // Check which services are already running (port-based detection)
    let is_running: Vec<bool> = services
        .iter()
        .map(|s| s.port.map(is_port_in_use).unwrap_or(false))
        .collect();

    // Find max name length for alignment
    let max_name_len = services.iter().map(|s| s.name.len()).max().unwrap_or(0);

    // Display services for selection
    let items: Vec<String> = services
        .iter()
        .zip(is_running.iter())
        .map(|(s, &running)| {
            let port_str = s.port
                .map(|p| format!("{}", p))
                .unwrap_or_else(|| "-".to_string());
            if running {
                format!(
                    "{:<width$}  {}  {}",
                    style(&s.name).dim(),
                    style(port_str).dim(),
                    style("(running)").dim().italic(),
                    width = max_name_len
                )
            } else {
                format!(
                    "{:<width$}  {}",
                    s.name,
                    style(port_str).dim(),
                    width = max_name_len
                )
            }
        })
        .collect();

    // Auto-select only services with detected ports that are not running
    let defaults: Vec<bool> = services
        .iter()
        .zip(is_running.iter())
        .map(|(s, &running)| s.port.is_some() && !running)
        .collect();

    // Check if all services are already running
    if is_running.iter().all(|&r| r) {
        println!(
            "{} All services are already running. Use {} to restart.",
            style("!").yellow(),
            style("gr restart").cyan()
        );
        return Ok(());
    }

    let theme = create_theme();
    let selections = MultiSelect::with_theme(&theme)
        .with_prompt("Select services to run")
        .items(&items)
        .defaults(&defaults)
        .interact_on(&Term::stderr())?;

    // Filter out already running services from selection
    let selections: Vec<usize> = selections
        .into_iter()
        .filter(|&i| !is_running[i])
        .collect();

    if selections.is_empty() {
        println!("{}", style("No services selected.").yellow());
        return Ok(());
    }

    let selected_services: Vec<&Service> = selections.iter().map(|&i| &services[i]).collect();

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

    // Spawn all selected services
    let mut handles: Vec<ProcessHandle> = Vec::new();
    for (idx, service) in selected_services.iter().enumerate() {
        let color = get_color_for_index(idx);

        match spawn_service(
            &service.name,
            &service.path,
            &service.dev_command,
            color.clone(),
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
    state.remove_project(&project_name);
    state.save()?;

    Ok(())
}
