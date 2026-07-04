use anyhow::{anyhow, Context, Result};
use console::style;
use dialoguer::{Confirm, FuzzySelect, Input};
use std::collections::HashSet;
use std::path::Path;

use crate::auth::provider;
use crate::discovery::{discover_services, discover_services_by_script, find_project_root};
use crate::ops::{generate_key_pair, OpsClient, OpsConfig, ServiceLink};
use crate::pass::storage::PassStorage;

/// Link a service to an ops application
pub async fn run_link(service: Option<String>) -> Result<()> {
    let auth = provider::get_valid_auth().await?;
    let master_password = rpassword::prompt_password("🔑 Master password: ")?;
    let root = find_project_root()?;

    // Discover all services (dev, build, or lint)
    let services = discover_all_services(&root)?;
    if services.is_empty() {
        return Err(anyhow!("No services found in project"));
    }

    let service_name = match service {
        Some(name) => {
            if !services.contains(&name) {
                return Err(anyhow!("Service '{}' not found", name));
            }
            name
        }
        None => {
            let selection = FuzzySelect::new()
                .with_prompt("Select service to link")
                .items(&services)
                .interact()?;
            services[selection].clone()
        }
    };

    // Get ops applications
    let client = OpsClient::new(auth.access_token.clone());
    let apps = client.list_apps().await?;

    if apps.is_empty() {
        println!(
            "{} No applications found. Create one at {}",
            style("!").yellow(),
            style("https://ops.groo.dev").cyan()
        );
        return Ok(());
    }

    // Select application
    let app_names: Vec<String> = apps.iter().map(|a| a.name.clone()).collect();
    let selection = FuzzySelect::new()
        .with_prompt("Select ops application")
        .items(&app_names)
        .interact()?;
    let app = &apps[selection];

    // Check if service is already linked
    let mut config = OpsConfig::load(&root)?;
    if let Some(existing) = config.get_service(&service_name) {
        if existing.application_id == app.id {
            println!(
                "{} Service '{}' is already linked to '{}'",
                style("✓").green(),
                service_name,
                app.name
            );
            return Ok(());
        }
        // Relink to different app
        println!(
            "Service '{}' is currently linked to '{}'. Relinking to '{}'...",
            service_name, existing.application_name, app.name
        );
    }

    // Select environment for secrets
    let env = select_environment()?;

    // Check if app has existing secrets setup
    let config_resp = client.get_config(&app.id, &env).await?;

    let key_pair = if config_resp.secrets_enabled {
        // App already has secrets - ask to import or create new
        println!(
            "\n{} This application already has secrets enabled for '{}'.",
            style("!").yellow(),
            env
        );
        println!("You can import the existing private key or create a new key pair.");
        println!(
            "{} Creating a new key pair will {}",
            style("Warning:").yellow(),
            style("delete all existing secrets").red()
        );

        let import = Confirm::new()
            .with_prompt("Import existing private key?")
            .default(true)
            .interact()?;

        if import {
            import_private_key(&app.id)?
        } else {
            let confirm = Confirm::new()
                .with_prompt("Create new key pair? (This will DELETE all existing secrets)")
                .default(false)
                .interact()?;

            if !confirm {
                println!("Cancelled.");
                return Ok(());
            }
            create_new_key_pair(&client, &app.id, &env, true).await?
        }
    } else {
        // No secrets yet - ask to enable
        let enable = Confirm::new()
            .with_prompt("Enable secrets encryption?")
            .default(true)
            .interact()?;

        if enable {
            create_new_key_pair(&client, &app.id, &env, false).await?
        } else {
            // Link without secrets
            (None, None)
        }
    };

    // Save link config
    let link = ServiceLink {
        application_id: app.id.clone(),
        application_name: app.name.clone(),
        public_key: key_pair.0,
    };
    config.set_service(service_name.clone(), link);
    config.save(&root)?;

    // Store private key in pass vault
    if let Some(private_key) = key_pair.1 {
        println!("{}", style("Storing private key in vault...").dim());
        let mut storage = PassStorage::unlock(&auth.access_token, &master_password).await?;
        storage.set_ops_key(&app.id, &private_key).await?;
    }

    println!(
        "\n{} Linked '{}' → '{}'",
        style("✓").green(),
        style(&service_name).cyan(),
        style(&app.name).cyan()
    );

    Ok(())
}

/// Unlink a service from ops
pub async fn run_unlink(service: Option<String>) -> Result<()> {
    let auth = provider::get_valid_auth().await?;
    let master_password = rpassword::prompt_password("🔑 Master password: ")?;
    let root = find_project_root()?;
    let mut config = OpsConfig::load(&root)?;

    if config.services.is_empty() {
        return Err(anyhow!("No services are linked"));
    }

    // Select service
    let service_name = match service {
        Some(name) => {
            if config.get_service(&name).is_none() {
                return Err(anyhow!("Service '{}' is not linked", name));
            }
            name
        }
        None => {
            let names: Vec<&str> = config.services.keys().map(|s| s.as_str()).collect();
            let selection = FuzzySelect::new()
                .with_prompt("Select service to unlink")
                .items(&names)
                .interact()?;
            names[selection].to_string()
        }
    };

    let link = config.get_service(&service_name).unwrap().clone();

    // Confirm
    let confirm = Confirm::new()
        .with_prompt(format!(
            "Unlink '{}' from '{}'?",
            service_name, link.application_name
        ))
        .default(false)
        .interact()?;

    if !confirm {
        println!("Cancelled.");
        return Ok(());
    }

    // Remove link
    config.remove_service(&service_name);
    config.save(&root)?;

    // Optionally remove private key from vault
    let storage = PassStorage::unlock(&auth.access_token, &master_password).await?;
    if storage.has_ops_key(&link.application_id) {
        let remove_key = Confirm::new()
            .with_prompt("Remove private key from vault?")
            .default(false)
            .interact()?;

        if remove_key {
            let mut storage = storage;
            storage.delete_ops_key(&link.application_id).await?;
            println!("Private key removed from vault.");
        }
    }

    println!(
        "{} Unlinked '{}' from '{}'",
        style("✓").green(),
        service_name,
        link.application_name
    );

    Ok(())
}

fn select_environment() -> Result<String> {
    let envs = ["development", "staging", "production"];
    let selection = FuzzySelect::new()
        .with_prompt("Select environment for secrets")
        .items(envs)
        .default(0)
        .interact()?;
    Ok(envs[selection].to_string())
}

fn import_private_key(_app_id: &str) -> Result<(Option<String>, Option<String>)> {
    println!("\nPaste your private key (base64-encoded PKCS8):");
    let private_key: String = Input::new().interact_text()?;
    let private_key = private_key.trim().to_string();

    // Validate it's valid base64
    use base64::{engine::general_purpose::STANDARD, Engine};
    STANDARD
        .decode(&private_key)
        .context("Invalid base64 encoding")?;

    // Key will be stored in vault by caller (run_link)
    println!(
        "{} Private key validated (will be stored in vault)",
        style("✓").green()
    );

    // We don't have the public key when importing, but that's OK
    // The server already has it
    Ok((None, Some(private_key)))
}

async fn create_new_key_pair(
    client: &OpsClient,
    app_id: &str,
    environment: &str,
    reset: bool,
) -> Result<(Option<String>, Option<String>)> {
    print!("Generating key pair... ");
    let key_pair = generate_key_pair()?;
    println!("{}", style("OK").green());

    // Upload public key to server
    print!("Uploading public key... ");
    if reset {
        client
            .reset_secrets(app_id, environment, &key_pair.public_key_jwk)
            .await?;
    } else {
        client
            .enable_secrets(app_id, environment, &key_pair.public_key_jwk)
            .await?;
    }
    println!("{}", style("OK").green());

    Ok((
        Some(key_pair.public_key_jwk),
        Some(key_pair.private_key_base64),
    ))
}

/// Discover all services that might need env vars (dev, build, or lint)
fn discover_all_services(root: &Path) -> Result<Vec<String>> {
    let mut names = HashSet::new();

    // Collect from dev, build, and lint scripts
    for service in discover_services(root).unwrap_or_default() {
        names.insert(service.name);
    }
    for service in discover_services_by_script(root, "build").unwrap_or_default() {
        names.insert(service.name);
    }
    for service in discover_services_by_script(root, "lint").unwrap_or_default() {
        names.insert(service.name);
    }

    let mut result: Vec<String> = names.into_iter().collect();
    result.sort();
    Ok(result)
}
