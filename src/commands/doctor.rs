use anyhow::Result;
use console::{style, Term};
use dialoguer::{Confirm, MultiSelect};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;

use crate::auth::storage::AuthState;
use crate::commands;
use crate::discovery::ports::FrameworkType;
use crate::discovery::{discover_services, discover_services_by_script, find_project_root};
use crate::ops::{has_private_key, OpsConfig};
use crate::project_config::ProjectConfig;

const PRE_COMMIT_HOOK: &str = r#"#!/bin/sh
# Groo pre-commit hook - lint services with staged changes
groo lint --changed
"#;

const PRE_PUSH_HOOK: &str = r#"#!/bin/sh
# Groo pre-push hook - build services with unpushed commits
groo build --changed
"#;

pub async fn run() -> Result<()> {
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

    // Collect fixable issues
    let mut fixable_hooks: Vec<(PathBuf, &'static str, &'static str)> = Vec::new();

    // 2. Discover services
    let dev_services = discover_services(&project_root).unwrap_or_default();
    let build_services = discover_services_by_script(&project_root, "build").unwrap_or_default();
    let lint_services = discover_services_by_script(&project_root, "lint").unwrap_or_default();

    // Combine all unique services: (port, has_dev, has_build, has_lint, framework)
    let mut all_services: HashMap<String, (Option<u16>, bool, bool, bool, FrameworkType)> =
        HashMap::new();

    for service in &dev_services {
        all_services
            .entry(service.name.clone())
            .or_insert((service.port, false, false, false, service.framework.clone()))
            .1 = true; // has dev
    }

    for service in &build_services {
        let entry = all_services
            .entry(service.name.clone())
            .or_insert((service.port, false, false, false, FrameworkType::Unknown));
        entry.2 = true; // has build
        if entry.0.is_none() {
            entry.0 = service.port;
        }
    }

    for service in &lint_services {
        let entry = all_services
            .entry(service.name.clone())
            .or_insert((service.port, false, false, false, FrameworkType::Unknown));
        entry.3 = true; // has lint
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

    // 4. Check each service has dev or build, and lint+build scripts
    let mut services_without_commands = Vec::new();
    let mut services_missing_lint = Vec::new();
    let mut services_missing_build = Vec::new();

    for (name, (_, has_dev, has_build, has_lint, _)) in &all_services {
        if !has_dev && !has_build {
            services_without_commands.push(name.clone());
        }
        if !has_lint {
            services_missing_lint.push(name.clone());
        }
        if !has_build {
            services_missing_build.push(name.clone());
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

    // Report missing lint scripts
    if services_missing_lint.is_empty() {
        println!("{} All services have lint scripts", style("✓").green());
    } else {
        for name in &services_missing_lint {
            println!(
                "{} Service '{}' missing 'lint' script",
                style("✗").red(),
                name
            );
        }
        has_errors = true;
    }

    // Report missing build scripts
    if services_missing_build.is_empty() {
        println!("{} All services have build scripts", style("✓").green());
    } else {
        for name in &services_missing_build {
            println!(
                "{} Service '{}' missing 'build' script",
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

    for (name, (port, has_dev, _, _, framework)) in &all_services {
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

    // 7. Check git hooks
    let hooks_dir = project_root.join(".git/hooks");
    let pre_commit_path = hooks_dir.join("pre-commit");
    let pre_push_path = hooks_dir.join("pre-push");

    if pre_commit_path.exists() {
        println!("{} pre-commit hook exists", style("✓").green());
    } else {
        println!("{} Missing pre-commit hook", style("✗").red());
        has_errors = true;
        fixable_hooks.push((pre_commit_path.clone(), PRE_COMMIT_HOOK, "pre-commit"));
    }

    if pre_push_path.exists() {
        println!("{} pre-push hook exists", style("✓").green());
    } else {
        println!("{} Missing pre-push hook", style("✗").red());
        has_errors = true;
        fixable_hooks.push((pre_push_path.clone(), PRE_PUSH_HOOK, "pre-push"));
    }

    // 8. Check auth status
    let auth = AuthState::load()?;
    if auth.is_none() {
        println!(
            "{} Not authenticated (run 'groo auth login')",
            style("!").yellow()
        );
        has_warnings = true;
    } else {
        let email = auth
            .as_ref()
            .and_then(|a| a.user_email.as_ref())
            .map(|e| e.as_str())
            .unwrap_or("unknown");
        println!("{} Authenticated as {}", style("✓").green(), email);
    }

    // 9. Check ops links + private keys
    let ops_config = OpsConfig::load(&project_root)?;
    let mut unlinked_services: Vec<String> = Vec::new();

    for service_name in all_services.keys() {
        match ops_config.get_service(service_name) {
            Some(link) => {
                // Linked - check for private key
                if !has_private_key(&link.application_id) {
                    println!(
                        "{} Service '{}' linked to ops ({}) but missing private key",
                        style("!").yellow(),
                        service_name,
                        link.application_name
                    );
                    has_warnings = true;
                }
            }
            None => {
                unlinked_services.push(service_name.clone());
            }
        }
    }

    if unlinked_services.is_empty() {
        println!("{} All services linked to ops", style("✓").green());
    } else {
        for name in &unlinked_services {
            println!(
                "{} Service '{}' not linked to ops",
                style("✗").red(),
                name
            );
        }
        has_errors = true;
    }

    // 10. Print services table
    if !all_services.is_empty() {
        println!();
        println!("{}", style("Services:").bold());

        let max_name_len = all_services.keys().map(|s| s.len()).max().unwrap_or(0);

        for (name, (port, has_dev, has_build, has_lint, _)) in &all_services {
            let port_str = port
                .map(|p| p.to_string())
                .unwrap_or_else(|| "-".to_string());

            let commands: Vec<&str> = [
                if *has_dev { Some("dev") } else { None },
                if *has_build { Some("build") } else { None },
                if *has_lint { Some("lint") } else { None },
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

    // Offer to fix hooks if any are missing
    if !fixable_hooks.is_empty() {
        println!();
        if Confirm::new()
            .with_prompt("Fix missing hooks?")
            .default(true)
            .interact_on(&Term::stderr())?
        {
            for (path, content, name) in &fixable_hooks {
                match create_hook(path, content) {
                    Ok(_) => println!("  {} Created {} hook", style("✓").green(), name),
                    Err(e) => println!(
                        "  {} Failed to create {} hook: {}",
                        style("✗").red(),
                        name,
                        e
                    ),
                }
            }
            println!();
            println!("{}", style("Hooks created!").green());
            // Re-evaluate has_errors since we may have fixed them
            has_errors = false;
            for (path, _, _) in &fixable_hooks {
                if !path.exists() {
                    has_errors = true;
                    break;
                }
            }
        }
    }

    // Offer to fix ops links if any are missing (only if authenticated)
    if !unlinked_services.is_empty() && auth.is_some() {
        println!();
        if Confirm::new()
            .with_prompt("Set up ops for services?")
            .default(true)
            .interact_on(&Term::stderr())?
        {
            // Sort for consistent display
            unlinked_services.sort();

            // Multi-select which services to link
            let selections = MultiSelect::new()
                .with_prompt("Select services to link")
                .items(&unlinked_services)
                .interact_on(&Term::stderr())?;

            if !selections.is_empty() {
                println!();
                for idx in selections {
                    let service = &unlinked_services[idx];
                    println!("{}", style(format!("Linking {}...", service)).bold());
                    if let Err(e) =
                        commands::ops::link::run_link(Some(service.clone())).await
                    {
                        println!(
                            "  {} Failed to link '{}': {}",
                            style("✗").red(),
                            service,
                            e
                        );
                    }
                    println!();
                }
                // Re-evaluate has_errors since we may have fixed some
                let updated_config = OpsConfig::load(&project_root)?;
                has_errors = all_services
                    .keys()
                    .any(|name| updated_config.get_service(name).is_none());
            }
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

fn create_hook(path: &PathBuf, content: &str) -> Result<()> {
    // Ensure hooks directory exists
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, content)?;

    // Make executable on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o755))?;
    }

    Ok(())
}
