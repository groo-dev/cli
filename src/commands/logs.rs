use anyhow::Result;
use console::{style, Style, Term};
use dialoguer::{theme::ColorfulTheme, MultiSelect};
use std::collections::VecDeque;
use std::io::{BufRead, Seek, SeekFrom};
use std::path::PathBuf;
use tokio::sync::broadcast;

use crate::config::get_service_log_file;
use crate::discovery::{discover_services, find_git_root, Service};
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
    let git_root = find_git_root()?;
    let services = discover_services(&git_root)?;

    // Filter to only running services (port-based detection)
    let running_services: Vec<&Service> = services
        .iter()
        .filter(|s| s.port.map(is_port_in_use).unwrap_or(false))
        .collect();

    if running_services.is_empty() {
        println!(
            "{} No running services found. Use {} to start services.",
            style("!").yellow(),
            style("gr dev").cyan()
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
        .with_prompt("Select services to view logs")
        .items(&items)
        .defaults(&defaults)
        .interact_on(&Term::stderr())?;

    if selections.is_empty() {
        println!("{}", style("No services selected.").yellow());
        return Ok(());
    }

    // Build list of selected services with their log files and colors
    let selected: Vec<ServiceLogInfo> = selections
        .iter()
        .map(|&i| {
            let service = running_services[i];
            ServiceLogInfo {
                name: service.name.clone(),
                log_file: get_service_log_file(&service.path),
                color: get_color_for_index(i),
            }
        })
        .collect();

    // Show last N lines from each service
    println!();
    for info in &selected {
        show_last_lines(&info.name, &info.log_file, &info.color, lines)?;
    }

    // If follow mode, stream new lines
    if follow {
        println!(
            "\n{} Following logs... (Ctrl+C to stop)\n",
            style("→").cyan().bold()
        );
        follow_logs(selected).await?;
    }

    Ok(())
}

fn show_last_lines(name: &str, log_file: &PathBuf, color: &Style, lines: usize) -> Result<()> {
    if !log_file.exists() {
        let prefix = color.apply_to(format!("[{}]", name));
        println!("{} {}", prefix, style("(no logs yet)").dim());
        return Ok(());
    }

    let file = std::fs::File::open(log_file)?;
    let reader = std::io::BufReader::new(file);

    // Read all lines and keep last N
    let mut last_lines: VecDeque<String> = VecDeque::with_capacity(lines);
    for line in reader.lines() {
        if let Ok(line) = line {
            if last_lines.len() >= lines {
                last_lines.pop_front();
            }
            last_lines.push_back(line);
        }
    }

    // Print each line with colored prefix
    for line in last_lines {
        // Log file format: [service] message, so just print directly
        let prefix = color.apply_to(format!("[{}]", name));
        // Remove [service] prefix from stored line if present
        let message = if line.starts_with('[') {
            if let Some(idx) = line.find(']') {
                line[idx + 1..].trim_start().to_string()
            } else {
                line
            }
        } else {
            line
        };
        println!("{} {}", prefix, message);
    }

    Ok(())
}

async fn follow_logs(services: Vec<ServiceLogInfo>) -> Result<()> {
    let (shutdown_tx, _) = broadcast::channel::<()>(1);

    // Set up Ctrl+C handler
    let shutdown_tx_clone = shutdown_tx.clone();
    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.ok();
        println!("\n{} Stopped following logs.", style("→").yellow().bold());
        let _ = shutdown_tx_clone.send(());
    });

    // Spawn a task for each service to tail its log file
    let mut handles = Vec::new();
    for info in services {
        let mut shutdown_rx = shutdown_tx.subscribe();
        let handle = tokio::spawn(async move {
            if let Err(e) = tail_log_file(&info.name, &info.log_file, &info.color, &mut shutdown_rx).await {
                let prefix = info.color.apply_to(format!("[{}]", info.name));
                eprintln!("{} Error: {}", prefix, e);
            }
        });
        handles.push(handle);
    }

    // Wait for all tasks to complete
    for handle in handles {
        let _ = handle.await;
    }

    Ok(())
}

async fn tail_log_file(
    name: &str,
    log_file: &PathBuf,
    color: &Style,
    shutdown_rx: &mut broadcast::Receiver<()>,
) -> Result<()> {
    // Wait for file to exist
    while !log_file.exists() {
        tokio::select! {
            _ = shutdown_rx.recv() => return Ok(()),
            _ = tokio::time::sleep(tokio::time::Duration::from_millis(100)) => {}
        }
    }

    // Open file and seek to end
    let file = tokio::fs::File::open(log_file).await?;
    let metadata = file.metadata().await?;
    let mut pos = metadata.len();

    loop {
        tokio::select! {
            _ = shutdown_rx.recv() => break,
            _ = tokio::time::sleep(tokio::time::Duration::from_millis(100)) => {
                // Check if file has grown
                let file = tokio::fs::File::open(log_file).await?;
                let metadata = file.metadata().await?;
                let new_len = metadata.len();

                if new_len > pos {
                    // Read new content
                    let mut file = std::fs::File::open(log_file)?;
                    file.seek(SeekFrom::Start(pos))?;

                    let reader = std::io::BufReader::new(file);
                    for line in reader.lines() {
                        if let Ok(line) = line {
                            let prefix = color.apply_to(format!("[{}]", name));
                            // Remove [service] prefix from stored line if present
                            let message = if line.starts_with('[') {
                                if let Some(idx) = line.find(']') {
                                    line[idx + 1..].trim_start().to_string()
                                } else {
                                    line
                                }
                            } else {
                                line
                            };
                            println!("{} {}", prefix, message);
                        }
                    }
                    pos = new_len;
                } else if new_len < pos {
                    // File was truncated (new session), reset position
                    pos = 0;
                }
            }
        }
    }

    Ok(())
}
