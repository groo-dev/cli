use std::process::Stdio;
use std::time::Instant;

use anyhow::Result;
use console::{style, Style, Term};
use dialoguer::{theme::ColorfulTheme, MultiSelect};
use futures::future::join_all;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

use crate::discovery::{
    discover_services_by_script, filter_services_with_changes, find_project_root,
    get_changed_files, Service,
};
use crate::project_config::ProjectConfig;
use crate::runner::{get_color_for_index, print_service_error, print_service_log};

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

struct LintResult {
    name: String,
    success: bool,
    duration: std::time::Duration,
    exit_code: Option<i32>,
}

pub async fn run(all: bool, changed: bool) -> Result<()> {
    let git_root = find_project_root()?;
    let services = discover_services_by_script(&git_root, "lint")?;

    if services.is_empty() {
        println!("{}", style("No services with lint scripts found.").yellow());
        return Ok(());
    }

    let selected_services: Vec<&Service> = if all {
        // --all: run on ALL services
        services.iter().collect()
    } else if changed {
        // --changed: only services with changes
        let changed_files = get_changed_files(&git_root)?;
        let affected = filter_services_with_changes(&services, &changed_files);

        if affected.is_empty() {
            println!("{}", style("No services with changes.").dim());
            return Ok(());
        }
        affected
    } else {
        // No flags: interactive picker
        select_services(&git_root, &services)?
    };

    if selected_services.is_empty() {
        println!("{}", style("No services selected.").yellow());
        return Ok(());
    }

    println!(
        "\n{} Linting {} service(s)...\n",
        style("→").green().bold(),
        selected_services.len()
    );

    // Spawn all lints in parallel
    let handles: Vec<_> = selected_services
        .iter()
        .enumerate()
        .map(|(idx, service)| {
            let name = service.name.clone();
            let path = service.path.clone();
            let command = service.dev_command.clone();
            let color = get_color_for_index(idx);
            tokio::spawn(async move { run_lint(&name, &path, &command, color).await })
        })
        .collect();

    // Wait for all lints to complete
    let results: Vec<LintResult> = join_all(handles)
        .await
        .into_iter()
        .filter_map(|r| r.ok())
        .collect();

    // Print summary
    println!();
    let mut succeeded = 0;
    let mut failed = 0;

    for result in &results {
        if result.success {
            succeeded += 1;
            println!(
                "{} {}  {:.1}s",
                style("✓").green().bold(),
                style(&result.name).green(),
                result.duration.as_secs_f64()
            );
        } else {
            failed += 1;
            let exit_info = result
                .exit_code
                .map(|c| format!("exit code {}", c))
                .unwrap_or_else(|| "failed".to_string());
            println!(
                "{} {}  ({})",
                style("✗").red().bold(),
                style(&result.name).red(),
                style(exit_info).dim()
            );
        }
    }

    println!();
    if failed > 0 {
        println!(
            "Lint completed: {} succeeded, {} failed",
            style(succeeded).green(),
            style(failed).red()
        );
        std::process::exit(1);
    } else {
        println!("Lint completed: {} succeeded", style(succeeded).green());
    }

    Ok(())
}

fn select_services<'a>(
    git_root: &std::path::Path,
    services: &'a [Service],
) -> Result<Vec<&'a Service>> {
    // Find max name length for alignment
    let max_name_len = services.iter().map(|s| s.name.len()).max().unwrap_or(0);

    // Display services for selection
    let items: Vec<String> = services
        .iter()
        .map(|s| format!("{:<width$}", s.name, width = max_name_len))
        .collect();

    // Load project config for saved selection
    let project_config = ProjectConfig::load(git_root).unwrap_or_default();

    // Use saved selection if available
    let defaults: Vec<bool> = if !project_config.selected_lint_services.is_empty() {
        services
            .iter()
            .map(|s| project_config.selected_lint_services.contains(&s.name))
            .collect()
    } else {
        // Default: select all
        vec![true; services.len()]
    };

    let theme = create_theme();
    let selections = MultiSelect::with_theme(&theme)
        .with_prompt("Select services to lint")
        .items(&items)
        .defaults(&defaults)
        .interact_on(&Term::stderr())?;

    if selections.is_empty() {
        return Ok(vec![]);
    }

    let selected: Vec<&Service> = selections.iter().map(|&i| &services[i]).collect();

    // Save selection for next time
    let mut project_config = ProjectConfig::load(git_root).unwrap_or_default();
    project_config.selected_lint_services = selected.iter().map(|s| s.name.clone()).collect();
    if let Err(e) = project_config.save(git_root) {
        eprintln!(
            "{} Failed to save selection: {}",
            style("⚠").yellow(),
            e
        );
    }

    Ok(selected)
}

async fn run_lint(name: &str, path: &std::path::Path, command: &str, color: Style) -> LintResult {
    let start = Instant::now();

    // For npm scripts, use "npm run lint"; for make, use command directly
    let full_command = if command.starts_with("make") {
        command.to_string()
    } else {
        "npm run lint".to_string()
    };

    let mut child = match Command::new("sh")
        .arg("-c")
        .arg(format!("cd {} && {}", path.display(), full_command))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(_) => {
            return LintResult {
                name: name.to_string(),
                success: false,
                duration: start.elapsed(),
                exit_code: None,
            };
        }
    };

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    // Spawn stdout reader
    let name_clone = name.to_string();
    let color_clone = color.clone();
    let stdout_handle = tokio::spawn(async move {
        if let Some(stdout) = stdout {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                print_service_log(&name_clone, &line, &color_clone);
            }
        }
    });

    // Spawn stderr reader
    let name_clone = name.to_string();
    let color_clone = color.clone();
    let stderr_handle = tokio::spawn(async move {
        if let Some(stderr) = stderr {
            let reader = BufReader::new(stderr);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                print_service_error(&name_clone, &line, &color_clone);
            }
        }
    });

    // Wait for process to complete
    let status = child.wait().await;
    let duration = start.elapsed();

    // Wait for output readers to finish
    let _ = stdout_handle.await;
    let _ = stderr_handle.await;

    match status {
        Ok(status) => LintResult {
            name: name.to_string(),
            success: status.success(),
            duration,
            exit_code: status.code(),
        },
        Err(_) => LintResult {
            name: name.to_string(),
            success: false,
            duration,
            exit_code: None,
        },
    }
}
