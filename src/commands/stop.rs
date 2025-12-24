use anyhow::Result;
use console::{style, Style, Term};
use dialoguer::{theme::ColorfulTheme, MultiSelect};

use crate::discovery::{discover_services, find_git_root, get_project_name, Service};
use crate::state::{is_port_in_use, State};

fn create_theme() -> ColorfulTheme {
    ColorfulTheme {
        defaults_style: Style::new().dim(),
        prompt_style: Style::new().bold(),
        prompt_prefix: style("?".to_string()).yellow().bold(),
        success_prefix: style("✓".to_string()).green().bold(),
        error_prefix: style("✗".to_string()).red().bold(),
        checked_item_prefix: style("  ◉".to_string()).red(),
        unchecked_item_prefix: style("  ○".to_string()).dim(),
        active_item_style: Style::new().yellow().bold(),
        inactive_item_style: Style::new().dim(),
        active_item_prefix: style("❯".to_string()).yellow().bold(),
        ..ColorfulTheme::default()
    }
}

pub fn run(project: Option<String>) -> Result<()> {
    let git_root = find_git_root()?;
    let project_name = project.unwrap_or_else(|| get_project_name(&git_root));
    let services = discover_services(&git_root)?;

    // Filter to only running services (port-based detection)
    let running_services: Vec<&Service> = services
        .iter()
        .filter(|s| s.port.map(is_port_in_use).unwrap_or(false))
        .collect();

    if running_services.is_empty() {
        println!(
            "{} No running services found for '{}'",
            style("!").yellow(),
            project_name
        );
        return Ok(());
    }

    // Find max name length for alignment
    let max_name_len = running_services.iter().map(|s| s.name.len()).max().unwrap_or(0);

    // Display running services for selection
    let items: Vec<String> = running_services
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
    let defaults: Vec<bool> = vec![true; running_services.len()];

    let theme = create_theme();
    let selections = MultiSelect::with_theme(&theme)
        .with_prompt("Select services to stop")
        .items(&items)
        .defaults(&defaults)
        .interact_on(&Term::stderr())?;

    if selections.is_empty() {
        println!("{}", style("No services selected.").yellow());
        return Ok(());
    }

    let selected_services: Vec<&Service> = selections
        .iter()
        .map(|&i| running_services[i])
        .collect();

    println!(
        "\n{} Stopping {} service(s)...\n",
        style("→").yellow().bold(),
        selected_services.len()
    );

    for service in &selected_services {
        if let Some(port) = service.port {
            let pids = get_pids_by_port(port);
            if pids.is_empty() {
                println!(
                    "  {} Could not find process for {}",
                    style("!").yellow(),
                    service.name
                );
            } else {
                let mut killed = false;
                for pid in &pids {
                    if kill_process(*pid) {
                        killed = true;
                    }
                }
                if killed {
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

    // Wait briefly for processes to terminate
    std::thread::sleep(std::time::Duration::from_millis(500));

    // Clean up state
    let mut state = State::load().unwrap_or_default();
    state.clean_stale_pids();
    state.save()?;

    println!(
        "\n{} Done.",
        style("✓").green().bold()
    );

    Ok(())
}

/// Get all PIDs of processes listening on a port using lsof
#[cfg(unix)]
pub fn get_pids_by_port(port: u16) -> Vec<u32> {
    use std::process::Command;
    let output = match Command::new("lsof")
        .args(["-ti", &format!(":{}", port)])
        .output()
    {
        Ok(o) => o,
        Err(_) => return vec![],
    };

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        stdout
            .lines()
            .filter_map(|line| line.trim().parse().ok())
            .collect()
    } else {
        vec![]
    }
}

#[cfg(not(unix))]
pub fn get_pids_by_port(port: u16) -> Vec<u32> {
    use std::process::Command;
    let output = match Command::new("netstat")
        .args(["-ano"])
        .output()
    {
        Ok(o) => o,
        Err(_) => return vec![],
    };

    let mut pids = vec![];
    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            if line.contains(&format!(":{}", port)) && line.contains("LISTENING") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if let Some(pid_str) = parts.last() {
                    if let Ok(pid) = pid_str.parse() {
                        pids.push(pid);
                    }
                }
            }
        }
    }
    pids
}

#[cfg(unix)]
pub fn kill_process(pid: u32) -> bool {
    use std::process::Command;

    // Try SIGTERM first
    let _ = Command::new("kill")
        .args(["-15", &pid.to_string()])
        .output();

    // Brief wait for graceful shutdown
    std::thread::sleep(std::time::Duration::from_millis(100));

    // Check if still running, if so use SIGKILL
    let still_running = Command::new("kill")
        .args(["-0", &pid.to_string()])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if still_running {
        Command::new("kill")
            .args(["-9", &pid.to_string()])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    } else {
        true
    }
}

#[cfg(not(unix))]
pub fn kill_process(pid: u32) -> bool {
    use std::process::Command;
    Command::new("taskkill")
        .args(["/F", "/PID", &pid.to_string()])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}
