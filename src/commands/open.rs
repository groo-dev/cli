use anyhow::Result;
use console::style;

use crate::discovery::{find_git_root, get_project_name};
use crate::state::State;

pub fn run(service_name: &str) -> Result<()> {
    let git_root = find_git_root()?;
    let project_name = get_project_name(&git_root);

    let state = State::load()?;

    let project_state = match state.get_project(&project_name) {
        Some(p) => p,
        None => {
            anyhow::bail!(
                "No running services found for project '{}'. Run 'gr dev' first.",
                project_name
            );
        }
    };

    let service = match project_state.services.get(service_name) {
        Some(s) => s,
        None => {
            let available: Vec<&str> = project_state.services.keys().map(|s| s.as_str()).collect();
            anyhow::bail!(
                "Service '{}' not found. Available services: {}",
                service_name,
                available.join(", ")
            );
        }
    };

    let port = match service.port {
        Some(p) => p,
        None => {
            anyhow::bail!("Service '{}' has no port configured", service_name);
        }
    };

    let url = format!("http://localhost:{}", port);
    println!(
        "{} Opening {} in browser...",
        style("→").green().bold(),
        style(&url).cyan()
    );

    open::that(&url)?;

    Ok(())
}
