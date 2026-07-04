use anyhow::{anyhow, Result};
use console::style;
use dialoguer::{Confirm, FuzzySelect};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::auth::provider;
use crate::discovery::{discover_services, discover_services_by_script, find_project_root, Service};
use crate::ops::{decrypt_secret, encrypt_secret, ConfigType, CreateConfigRequest, OpsClient, OpsConfig};
use crate::pass::storage::PassStorage;

/// List env vars and secrets for a linked service
pub async fn run_list(service: Option<String>, environment: String) -> Result<()> {
    let (client, config, service_name, _root, storage) = setup(service).await?;

    let link = config
        .get_service(&service_name)
        .ok_or_else(|| anyhow!("Service '{}' is not linked", service_name))?;

    let resp = client.get_config(&link.application_id, &environment).await?;

    println!(
        "\n{} {} ({} environment)\n",
        style(&link.application_name).cyan().bold(),
        style(&service_name).dim(),
        style(&environment).yellow()
    );

    if resp.variables.is_empty() && resp.secrets.is_empty() {
        println!("{}", style("No configuration found.").dim());
        return Ok(());
    }

    // Print variables
    if !resp.variables.is_empty() {
        println!("{}", style("Variables:").bold());
        for var in &resp.variables {
            println!("  {} = {}", style(&var.name).green(), var.value);
        }
        println!();
    }

    // Print secrets (decrypted)
    if !resp.secrets.is_empty() {
        println!("{}", style("Secrets:").bold());

        let private_key = storage.get_ops_key(&link.application_id);

        for secret in &resp.secrets {
            let value = if let Some(ref pk) = private_key {
                match decrypt_secret(&secret.value, pk) {
                    Ok(v) => v,
                    Err(_) => style("(decryption failed)").red().to_string(),
                }
            } else {
                style("(no private key)").dim().to_string()
            };
            println!("  {} = {}", style(&secret.name).magenta(), value);
        }
        println!();
    }

    Ok(())
}

/// Show diff between local env file and remote config
pub async fn run_diff(service: Option<String>, environment: String) -> Result<()> {
    let (client, config, service_name, root, storage) = setup(service).await?;

    let link = config
        .get_service(&service_name)
        .ok_or_else(|| anyhow!("Service '{}' is not linked", service_name))?;

    // Find service path
    let service_info = find_service_by_name(&root, &service_name)
        .ok_or_else(|| anyhow!("Service '{}' not found", service_name))?;

    // Get local env file
    let env_file = detect_env_file(&service_info.path);
    let local = parse_env_file(&env_file).unwrap_or_default();

    // Get remote config
    let resp = client.get_config(&link.application_id, &environment).await?;
    let private_key = storage.get_ops_key(&link.application_id);

    // Build remote map
    let mut remote: HashMap<String, (String, bool)> = HashMap::new();
    for var in &resp.variables {
        remote.insert(var.name.clone(), (var.value.clone(), false));
    }
    for secret in &resp.secrets {
        let value = if let Some(ref pk) = private_key {
            decrypt_secret(&secret.value, pk).unwrap_or_else(|_| "(encrypted)".to_string())
        } else {
            "(encrypted)".to_string()
        };
        remote.insert(secret.name.clone(), (value, true));
    }

    println!(
        "\n{} {} vs {}\n",
        style("Diff:").bold(),
        style(env_file.file_name().unwrap_or_default().to_string_lossy()).cyan(),
        style(format!("ops:{}", environment)).yellow()
    );

    let mut has_diff = false;

    // Check for differences
    let mut all_keys: Vec<String> = local.keys().chain(remote.keys()).cloned().collect();
    all_keys.sort();
    all_keys.dedup();

    for key in all_keys {
        let local_val = local.get(&key);
        let remote_entry = remote.get(&key);

        match (local_val, remote_entry) {
            (Some(lv), Some((rv, is_secret))) => {
                if lv != rv {
                    has_diff = true;
                    let type_indicator = if *is_secret {
                        style("[secret]").magenta()
                    } else {
                        style("[var]").green()
                    };
                    println!("{} {} {}", style("~").yellow(), key, type_indicator);
                    println!("  {} {}", style("-").red(), lv);
                    println!("  {} {}", style("+").green(), rv);
                }
            }
            (Some(lv), None) => {
                has_diff = true;
                println!("{} {} {}", style("-").red(), key, style("(local only)").dim());
                println!("  {}", lv);
            }
            (None, Some((rv, is_secret))) => {
                has_diff = true;
                let type_indicator = if *is_secret {
                    style("[secret]").magenta()
                } else {
                    style("[var]").green()
                };
                println!(
                    "{} {} {} {}",
                    style("+").green(),
                    key,
                    type_indicator,
                    style("(remote only)").dim()
                );
                println!("  {}", rv);
            }
            (None, None) => unreachable!(),
        }
    }

    if !has_diff {
        println!("{}", style("No differences found.").green());
    }

    Ok(())
}

/// Pull remote config to local env file
pub async fn run_pull(service: Option<String>, environment: String) -> Result<()> {
    let (client, config, service_name, root, storage) = setup(service).await?;

    let link = config
        .get_service(&service_name)
        .ok_or_else(|| anyhow!("Service '{}' is not linked", service_name))?;

    // Find service path
    let service_info = find_service_by_name(&root, &service_name)
        .ok_or_else(|| anyhow!("Service '{}' not found", service_name))?;

    let env_file = detect_env_file(&service_info.path);

    // Get remote config
    let resp = client.get_config(&link.application_id, &environment).await?;
    let private_key = storage.get_ops_key(&link.application_id);

    // Build env file content
    let mut lines: Vec<String> = Vec::new();

    // Variables first
    if !resp.variables.is_empty() {
        lines.push("# Variables".to_string());
        for var in &resp.variables {
            lines.push(format!("{}={}", var.name, var.value));
        }
    }

    // Then secrets
    if !resp.secrets.is_empty() {
        if !lines.is_empty() {
            lines.push(String::new());
        }
        lines.push("# Secrets".to_string());
        for secret in &resp.secrets {
            let value = if let Some(ref pk) = private_key {
                decrypt_secret(&secret.value, pk)
                    .unwrap_or_else(|_| "# DECRYPTION_FAILED".to_string())
            } else {
                "# NO_PRIVATE_KEY".to_string()
            };
            lines.push(format!("{}={}", secret.name, value));
        }
    }

    if lines.is_empty() {
        println!("{}", style("No configuration to pull.").yellow());
        return Ok(());
    }

    // Confirm overwrite
    if env_file.exists() {
        let confirm = Confirm::new()
            .with_prompt(format!(
                "Overwrite {}?",
                env_file.file_name().unwrap_or_default().to_string_lossy()
            ))
            .default(false)
            .interact()?;

        if !confirm {
            println!("Cancelled.");
            return Ok(());
        }
    }

    // Write file
    let content = lines.join("\n") + "\n";
    fs::write(&env_file, content)?;

    println!(
        "{} Wrote {} entries to {}",
        style("✓").green(),
        resp.variables.len() + resp.secrets.len(),
        style(env_file.display()).cyan()
    );

    Ok(())
}

/// Push local env file to remote
pub async fn run_push(service: Option<String>, environment: String) -> Result<()> {
    let (client, config, service_name, root, _storage) = setup(service).await?;

    let link = config
        .get_service(&service_name)
        .ok_or_else(|| anyhow!("Service '{}' is not linked", service_name))?;

    // Find service path
    let service_info = find_service_by_name(&root, &service_name)
        .ok_or_else(|| anyhow!("Service '{}' not found", service_name))?;

    let env_file = detect_env_file(&service_info.path);
    let local = parse_env_file(&env_file)?;

    if local.is_empty() {
        println!("{}", style("No variables in local env file.").yellow());
        return Ok(());
    }

    // Get remote config to compare
    let resp = client.get_config(&link.application_id, &environment).await?;

    let mut remote_vars: HashMap<String, String> = HashMap::new();
    let mut remote_secrets: HashMap<String, String> = HashMap::new();
    let mut config_ids: HashMap<String, String> = HashMap::new();

    for var in &resp.variables {
        remote_vars.insert(var.name.clone(), var.value.clone());
        config_ids.insert(var.name.clone(), var.id.clone());
    }
    for secret in &resp.secrets {
        remote_secrets.insert(secret.name.clone(), secret.value.clone());
        config_ids.insert(secret.name.clone(), secret.id.clone());
    }

    // Get public key for encrypting secrets
    let public_key = link.public_key.as_ref().or(resp.public_key.as_ref());

    println!(
        "\n{} Pushing to {} ({})\n",
        style(&link.application_name).cyan().bold(),
        style(&environment).yellow(),
        style(local.len()).dim()
    );

    let mut created = 0;
    let mut updated = 0;
    let mut skipped = 0;

    for (name, value) in &local {
        // Check if exists
        let existing_var = remote_vars.get(name);
        let existing_secret = remote_secrets.get(name);

        if existing_var.is_some() || existing_secret.is_some() {
            // Update existing
            let config_id = config_ids.get(name).unwrap();

            // Check if value actually changed (for variables)
            if let Some(ev) = existing_var
                && ev == value
            {
                skipped += 1;
                continue;
            }

            print!("  Updating {}... ", style(name).cyan());
            let new_value = if existing_secret.is_some() {
                // Re-encrypt for secrets
                if let Some(pk) = public_key {
                    encrypt_secret(value, pk)?
                } else {
                    return Err(anyhow!(
                        "Cannot update secret '{}': no public key available",
                        name
                    ));
                }
            } else {
                value.clone()
            };

            client
                .update_config(&link.application_id, config_id, new_value)
                .await?;
            println!("{}", style("OK").green());
            updated += 1;
        } else {
            // New entry - ask if it's a secret
            let is_secret = prompt_is_secret(name)?;

            print!("  Creating {} {}... ", style(name).cyan(), if is_secret { style("[secret]").magenta() } else { style("[var]").green() });

            let final_value = if is_secret {
                if let Some(pk) = public_key {
                    encrypt_secret(value, pk)?
                } else {
                    return Err(anyhow!(
                        "Cannot create secret '{}': secrets not enabled for this environment",
                        name
                    ));
                }
            } else {
                value.clone()
            };

            let config_type = if is_secret {
                ConfigType::Secret
            } else {
                ConfigType::Variable
            };

            client
                .create_config(
                    &link.application_id,
                    CreateConfigRequest {
                        config_type,
                        environment: environment.clone(),
                        name: name.clone(),
                        value: final_value,
                    },
                )
                .await?;
            println!("{}", style("OK").green());
            created += 1;
        }
    }

    println!(
        "\n{} {} created, {} updated, {} unchanged",
        style("✓").green(),
        created,
        updated,
        skipped
    );

    Ok(())
}

/// Setup common state for env commands
async fn setup(
    service: Option<String>,
) -> Result<(OpsClient, OpsConfig, String, PathBuf, PassStorage)> {
    let auth = provider::get_valid_auth().await?;
    let master_password = rpassword::prompt_password("🔑 Master password: ")?;
    let root = find_project_root()?;
    let config = OpsConfig::load(&root)?;

    if config.services.is_empty() {
        return Err(anyhow!(
            "No services linked. Run 'groo ops link' first."
        ));
    }

    // Select service
    let service_name = match service {
        Some(name) => {
            if config.get_service(&name).is_none() {
                return Err(anyhow!("Service '{}' is not linked", name));
            }
            name
        }
        None if config.services.len() == 1 => {
            config.services.keys().next().unwrap().clone()
        }
        None => {
            let names: Vec<&str> = config.services.keys().map(|s| s.as_str()).collect();
            let selection = FuzzySelect::new()
                .with_prompt("Select service")
                .items(&names)
                .interact()?;
            names[selection].to_string()
        }
    };

    let client = OpsClient::new(auth.access_token.clone());

    // Unlock pass vault for key access
    println!("{}", style("Unlocking vault...").dim());
    let storage = PassStorage::unlock(&auth.access_token, &master_password).await?;

    Ok((client, config, service_name, root, storage))
}

/// Detect which env file to use based on service type
fn detect_env_file(service_dir: &Path) -> PathBuf {
    // Cloudflare Workers use .dev.vars
    if service_dir.join("wrangler.toml").exists() || service_dir.join("wrangler.jsonc").exists() {
        return service_dir.join(".dev.vars");
    }
    // Default to .env.development
    service_dir.join(".env.development")
}

/// Parse env file into key-value map
fn parse_env_file(path: &Path) -> Result<HashMap<String, String>> {
    if !path.exists() {
        return Err(anyhow!("Env file not found: {}", path.display()));
    }

    let content = fs::read_to_string(path)?;
    let mut map = HashMap::new();

    for line in content.lines() {
        let line = line.trim();

        // Skip empty lines and comments
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        // Parse KEY=value
        if let Some(eq_pos) = line.find('=') {
            let key = line[..eq_pos].trim().to_string();
            let value = line[eq_pos + 1..].trim().to_string();

            // Remove quotes if present
            let value = if (value.starts_with('"') && value.ends_with('"'))
                || (value.starts_with('\'') && value.ends_with('\''))
            {
                value[1..value.len() - 1].to_string()
            } else {
                value
            };

            map.insert(key, value);
        }
    }

    Ok(map)
}

/// Find a service by name from any script type (dev, build, or lint)
fn find_service_by_name(root: &Path, name: &str) -> Option<Service> {
    // Try dev services first
    if let Ok(services) = discover_services(root)
        && let Some(s) = services.into_iter().find(|s| s.name == name)
    {
        return Some(s);
    }
    // Try build services
    if let Ok(services) = discover_services_by_script(root, "build")
        && let Some(s) = services.into_iter().find(|s| s.name == name)
    {
        return Some(s);
    }
    // Try lint services
    if let Ok(services) = discover_services_by_script(root, "lint")
        && let Some(s) = services.into_iter().find(|s| s.name == name)
    {
        return Some(s);
    }
    None
}

/// Prompt user to classify a variable as secret or not
fn prompt_is_secret(name: &str) -> Result<bool> {
    // Auto-detect common secret patterns
    let lower = name.to_lowercase();
    let likely_secret = lower.contains("secret")
        || lower.contains("key")
        || lower.contains("token")
        || lower.contains("password")
        || lower.contains("api_key")
        || lower.contains("apikey")
        || lower.contains("auth")
        || lower.contains("credential");

    let prompt = if likely_secret {
        format!("Is '{}' a secret? (likely yes)", name)
    } else {
        format!("Is '{}' a secret?", name)
    };

    Confirm::new()
        .with_prompt(prompt)
        .default(likely_secret)
        .interact()
        .map_err(Into::into)
}
