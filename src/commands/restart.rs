use anyhow::Result;
use console::{style, Style, Term};
use dialoguer::{theme::ColorfulTheme, MultiSelect};

use crate::discovery::{discover_services, find_project_root, Service};
use crate::state::is_port_in_use;

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
    let services = discover_services(&git_root)?;

    let running_services: Vec<&Service> = services
        .iter()
        .filter(|s| s.port.map(is_port_in_use).unwrap_or(false))
        .collect();

    if running_services.is_empty() {
        println!(
            "{} No running services found. Use {} to start services.",
            style("!").yellow(),
            style("groo dev").cyan()
        );
        return Ok(());
    }

    let max_name_len = running_services.iter().map(|s| s.name.len()).max().unwrap_or(0);

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

    let defaults: Vec<bool> = vec![true; running_services.len()];

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
        .map(|&i| running_services[i])
        .collect();

    println!(
        "\n{} Stopping {} service(s)...\n",
        style("→").yellow().bold(),
        selected_services.len()
    );

    for service in &selected_services {
        if let Some(port) = service.port
            && let Some(pid) = get_pid_by_port(port) {
                if kill_process(pid) {
                    println!("  {} Stopped {}", style("✓").green(), service.name);
                } else {
                    println!("  {} Failed to stop {}", style("✗").red(), service.name);
                }
            }
    }

    println!(
        "\n{} Services stopped. Use {} to start them again.",
        style("✓").green().bold(),
        style("groo dev").cyan()
    );

    Ok(())
}

#[cfg(unix)]
fn get_pid_by_port(port: u16) -> Option<u32> {
    use std::process::Command;
    let output = Command::new("lsof")
        .args(["-ti", &format!(":{}", port)])
        .output()
        .ok()?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
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
