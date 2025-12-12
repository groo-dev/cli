use anyhow::Result;
use console::style;

use crate::discovery::{discover_services, find_git_root, get_project_name};
use crate::state::is_port_in_use;

pub fn run(project: Option<String>) -> Result<()> {
    let git_root = find_git_root()?;
    let project_name = project.unwrap_or_else(|| get_project_name(&git_root));

    // Discover all services
    let services = discover_services(&git_root)?;

    if services.is_empty() {
        println!(
            "{} No services with dev scripts found in '{}'",
            style("!").yellow(),
            project_name
        );
        return Ok(());
    }

    // Find max name length for alignment
    let max_name_len = services.iter().map(|s| s.name.len()).max().unwrap_or(0);

    println!("{}", style(&project_name).cyan().bold());
    println!();

    // Print header
    println!(
        "  {:<width$}  {:<6} {}",
        style("Service").bold(),
        style("Port").bold(),
        style("Status").bold(),
        width = max_name_len
    );
    println!("  {}", "-".repeat(max_name_len + 20));

    // Print all discovered services
    for service in &services {
        let port_str = service
            .port
            .map(|p| p.to_string())
            .unwrap_or_else(|| "-".to_string());

        // Check if this service is running (port-based)
        let status = match service.port {
            Some(port) if is_port_in_use(port) => style("Running").green(),
            _ => style("Stopped").dim(),
        };

        println!(
            "  {:<width$}  {:<6} {}",
            service.name,
            port_str,
            status,
            width = max_name_len
        );
    }

    Ok(())
}
