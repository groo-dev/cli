use anyhow::Result;
use console::style;

use crate::state::State;

pub fn run() -> Result<()> {
    let mut state = State::load()?;
    state.clean_stale_pids();
    state.save()?;

    if state.projects.is_empty() {
        println!("{}", style("No projects with running services.").yellow());
        return Ok(());
    }

    println!("{}", style("Projects with running services:").bold());
    println!();

    for (name, project) in &state.projects {
        let service_count = project.services.len();
        let suffix = if service_count == 1 { "service" } else { "services" };
        println!(
            "  {} {} ({} {})",
            style("●").green(),
            style(name).cyan().bold(),
            service_count,
            suffix
        );
    }

    Ok(())
}
