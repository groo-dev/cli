use anyhow::Result;
use console::style;
use std::collections::{HashMap, HashSet};

use crate::discovery::ports::FrameworkType;
use crate::discovery::{discover_services, discover_services_by_script, find_project_root};
use crate::project_config::ProjectConfig;

pub fn run() -> Result<()> {
    let mut has_errors = false;
    let mut has_warnings = false;

    // 1. Detect project root
    let project_root = match find_project_root() {
        Ok(root) => {
            let via = if root.join(".groo").is_dir() {
                ".groo/"
            } else {
                ".git/"
            };
            println!(
                "{} Project root: {} (via {})",
                style("✓").green(),
                root.display(),
                via
            );
            root
        }
        Err(_) => {
            println!(
                "{} Project root: not found (no .groo/ or .git/ directory)",
                style("✗").red()
            );
            return Ok(());
        }
    };

    // 2. Discover services
    let dev_services = discover_services(&project_root).unwrap_or_default();
    let build_services = discover_services_by_script(&project_root, "build").unwrap_or_default();

    // Combine all unique services: (port, has_dev, has_build, framework)
    let mut all_services: HashMap<String, (Option<u16>, bool, bool, FrameworkType)> = HashMap::new();

    for service in &dev_services {
        all_services
            .entry(service.name.clone())
            .or_insert((service.port, false, false, service.framework.clone()))
            .1 = true; // has dev
    }

    for service in &build_services {
        let entry = all_services
            .entry(service.name.clone())
            .or_insert((service.port, false, false, FrameworkType::Unknown));
        entry.2 = true; // has build
        if entry.0.is_none() {
            entry.0 = service.port;
        }
    }

    // 3. Check at least one service found
    if all_services.is_empty() {
        println!(
            "{} No services found (need package.json with scripts or Makefile)",
            style("✗").red()
        );
        has_errors = true;
    } else {
        println!(
            "{} Found {} service(s)",
            style("✓").green(),
            all_services.len()
        );
    }

    // 4. Check each service has dev or build
    let mut services_without_commands = Vec::new();
    for (name, (_, has_dev, has_build, _)) in &all_services {
        if !has_dev && !has_build {
            services_without_commands.push(name.clone());
        }
    }

    if !services_without_commands.is_empty() {
        for name in &services_without_commands {
            println!(
                "{} Service '{}' has no dev or build command",
                style("✗").red(),
                name
            );
        }
        has_errors = true;
    }

    // 5. Check ports (only for server frameworks with dev command)
    let mut port_issues = Vec::new();
    let mut missing_ports = Vec::new();
    let mut port_to_services: HashMap<u16, Vec<String>> = HashMap::new();

    for (name, (port, has_dev, _, framework)) in &all_services {
        // Only check ports for dev services (build-only services don't need ports)
        if !has_dev {
            continue;
        }

        // Only check ports for server frameworks
        let is_server_framework = matches!(
            framework,
            FrameworkType::NextJs | FrameworkType::Vite | FrameworkType::Wrangler
        );

        match port {
            Some(p) => {
                // Check 5-digit requirement (10000-65535) only for server frameworks
                if is_server_framework && *p < 10000 {
                    port_issues.push((name.clone(), *p, "port should be 5 digits (>= 10000)"));
                }

                // Track for duplicate detection (only for server frameworks)
                if is_server_framework {
                    port_to_services
                        .entry(*p)
                        .or_default()
                        .push(name.clone());
                }
            }
            None => {
                // Only warn about missing ports for server frameworks
                if is_server_framework {
                    missing_ports.push(name.clone());
                }
            }
        }
    }

    // Report port issues
    for (name, port, issue) in &port_issues {
        println!(
            "{} Service '{}' port {}: {}",
            style("✗").red(),
            name,
            port,
            issue
        );
        has_errors = true;
    }

    // Check for duplicate ports
    for (port, services) in &port_to_services {
        if services.len() > 1 {
            println!(
                "{} Port {} is used by multiple services: {}",
                style("✗").red(),
                port,
                services.join(", ")
            );
            has_errors = true;
        }
    }

    // Report missing ports (warning)
    for name in &missing_ports {
        println!(
            "{} Service '{}' has no port configured",
            style("!").yellow(),
            name
        );
        has_warnings = true;
    }

    // 6. Check for stale selections in config
    if let Ok(config) = ProjectConfig::load(&project_root) {
        let service_names: HashSet<_> = all_services.keys().cloned().collect();

        let stale_dev: Vec<_> = config
            .selected_services
            .iter()
            .filter(|s| !service_names.contains(*s))
            .collect();

        let stale_build: Vec<_> = config
            .selected_build_services
            .iter()
            .filter(|s| !service_names.contains(*s))
            .collect();

        if !stale_dev.is_empty() {
            println!(
                "{} Stale dev selections in config: {}",
                style("!").yellow(),
                stale_dev.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ")
            );
            has_warnings = true;
        }

        if !stale_build.is_empty() {
            println!(
                "{} Stale build selections in config: {}",
                style("!").yellow(),
                stale_build.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ")
            );
            has_warnings = true;
        }
    }

    // 7. Print services table
    if !all_services.is_empty() {
        println!();
        println!("{}", style("Services:").bold());

        let max_name_len = all_services.keys().map(|s| s.len()).max().unwrap_or(0);

        for (name, (port, has_dev, has_build, _)) in &all_services {
            let port_str = port
                .map(|p| p.to_string())
                .unwrap_or_else(|| "-".to_string());

            let commands: Vec<&str> = [
                if *has_dev { Some("dev") } else { None },
                if *has_build { Some("build") } else { None },
            ]
            .into_iter()
            .flatten()
            .collect();

            println!(
                "  {:<width$}  {:>5}  {}",
                name,
                style(port_str).dim(),
                style(commands.join(", ")).dim(),
                width = max_name_len
            );
        }
    }

    // Summary
    println!();
    if has_errors {
        println!("{}", style("Issues found. Please fix the errors above.").red());
        std::process::exit(1);
    } else if has_warnings {
        println!("{}", style("All checks passed with warnings.").yellow());
    } else {
        println!("{}", style("All checks passed.").green());
    }

    Ok(())
}
