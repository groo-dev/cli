use anyhow::Result;
use console::{style, Style, Term};
use dialoguer::{theme::ColorfulTheme, Confirm, MultiSelect};

use crate::commands::stop::{get_pids_by_port, kill_process};
use crate::dev_tmux;
use crate::discovery::{discover_services, find_project_root, get_project_name, Service};
use crate::project_config::ProjectConfig;
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
    let git_root = find_project_root()?;
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
    let mut is_running: Vec<bool> = services
        .iter()
        .map(|s| s.port.map(is_port_in_use).unwrap_or(false))
        .collect();

    // Collect running services
    let running_services: Vec<(&Service, usize)> = services
        .iter()
        .enumerate()
        .filter(|(i, _)| is_running[*i])
        .map(|(i, s)| (s, i))
        .collect();

    // Prompt to stop if any are running
    if !running_services.is_empty() {
        println!("{}", style("Running services:").yellow().bold());
        for (service, _) in &running_services {
            let port_str = service
                .port
                .map(|p| format!(":{}", p))
                .unwrap_or_default();
            println!(
                "  {} {}",
                style(&service.name).cyan(),
                style(port_str).dim()
            );
        }
        println!();

        let stop_them = Confirm::new()
            .with_prompt("Stop running services?")
            .default(true)
            .interact()?;

        if stop_them {
            for (service, _) in &running_services {
                if let Some(port) = service.port {
                    for pid in get_pids_by_port(port) {
                        kill_process(pid);
                    }
                    println!("  {} Stopped {}", style("✓").green(), service.name);
                }
            }
            // Brief wait for ports to be released
            std::thread::sleep(std::time::Duration::from_millis(300));

            // Refresh running status
            is_running = services
                .iter()
                .map(|s| s.port.map(is_port_in_use).unwrap_or(false))
                .collect();
            println!();
        }
    }

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

    // Load project config for saved selection
    let project_config = ProjectConfig::load(&git_root).unwrap_or_default();

    // Use saved selection if available, otherwise fall back to port-based defaults
    let defaults: Vec<bool> = if !project_config.selected_services.is_empty() {
        services
            .iter()
            .zip(is_running.iter())
            .map(|(s, &running)| {
                !running && project_config.selected_services.contains(&s.name)
            })
            .collect()
    } else {
        // Fall back: auto-select services with detected ports that are not running
        services
            .iter()
            .zip(is_running.iter())
            .map(|(s, &running)| s.port.is_some() && !running)
            .collect()
    };

    let theme = create_theme();
    let selections = MultiSelect::with_theme(&theme)
        .with_prompt("Select services to run")
        .items(&items)
        .defaults(&defaults)
        .interact_on(&Term::stderr())?;

    if selections.is_empty() {
        println!("{}", style("No services selected.").yellow());
        return Ok(());
    }

    let selected_services: Vec<Service> = selections
        .iter()
        .map(|&i| services[i].clone())
        .collect();

    // Save selection for next time
    let mut project_config = ProjectConfig::load(&git_root).unwrap_or_default();
    project_config.selected_services = selected_services.iter().map(|s| s.name.clone()).collect();
    if let Err(e) = project_config.save(&git_root) {
        eprintln!(
            "{} Failed to save selection: {}",
            style("⚠").yellow(),
            e
        );
    }

    // Launch tmux session with selected services
    dev_tmux::run(project_name.clone(), git_root.clone(), selected_services).await?;

    Ok(())
}
