use anyhow::Result;
use console::{style, Style, Term};
use dialoguer::{theme::ColorfulTheme, MultiSelect};
use std::path::PathBuf;
use tokio::sync::broadcast;

use crate::config::get_service_log_file;
use crate::discovery::{discover_services, find_project_root, get_project_name, Service};
use crate::log_tailer;
use crate::runner::get_color_for_index;
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

struct ServiceLogInfo {
    name: String,
    log_file: PathBuf,
    color: Style,
}

pub async fn run(lines: usize, follow: bool) -> Result<()> {
    let git_root = find_project_root()?;
    let project_name = get_project_name(&git_root);
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
        .with_prompt("Select services to view logs")
        .items(&items)
        .defaults(&defaults)
        .interact_on(&Term::stderr())?;

    if selections.is_empty() {
        println!("{}", style("No services selected.").yellow());
        return Ok(());
    }

    let selected: Vec<ServiceLogInfo> = selections
        .iter()
        .map(|&i| {
            let service = running_services[i];
            ServiceLogInfo {
                name: service.name.clone(),
                log_file: get_service_log_file(&project_name, &service.name),
                color: get_color_for_index(i),
            }
        })
        .collect();

    println!();
    for info in &selected {
        log_tailer::show_last_lines(&info.name, &info.log_file, &info.color, lines)?;
    }

    if follow {
        println!(
            "\n{} Following logs... (Ctrl+C to stop)\n",
            style("→").cyan().bold()
        );
        follow_logs(selected).await?;
    }

    Ok(())
}

async fn follow_logs(services: Vec<ServiceLogInfo>) -> Result<()> {
    let (shutdown_tx, _) = broadcast::channel::<()>(1);

    let shutdown_tx_clone = shutdown_tx.clone();
    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.ok();
        println!("\n{} Stopped following logs.", style("→").yellow().bold());
        let _ = shutdown_tx_clone.send(());
    });

    let mut handles = Vec::new();
    for info in services {
        let mut shutdown_rx = shutdown_tx.subscribe();
        let handle = tokio::spawn(async move {
            if let Err(e) = log_tailer::tail_log_file(
                &info.name, &info.log_file, &info.color, &mut shutdown_rx,
            ).await {
                let prefix = info.color.apply_to(format!("[{}]", info.name));
                eprintln!("{} Error: {}", prefix, e);
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        let _ = handle.await;
    }

    Ok(())
}
