use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;
use console::style;
use dialoguer::Confirm;
use keyring::Entry;
use uuid::Uuid;

use crate::auth::storage::load_auth_with_password;
use crate::discovery::find_project_root;
use crate::ops::OpsConfig;
use crate::pass::client::PassClient;
use crate::pass::types::{NoteItem, VaultItem};

const SERVICE_NAME: &str = "groo-cli";
const CLI_OPS_PREFIX: &str = "groo-cli:ops:";

pub async fn run() -> Result<()> {
    println!("{}", style("Groo CLI Secret Migration").bold());
    println!();
    println!("This will migrate your ops private keys from macOS Keychain to Groo Pass.");
    println!("(Note: Auth tokens are now stored locally encrypted with your master password)");
    println!();

    // Find ops keys in keychain
    let ops_keys = find_ops_keys();

    if ops_keys.is_empty() {
        println!("{} No ops keys found in keychain to migrate.", style("!").yellow());
        return Ok(());
    }

    // Show what will be migrated
    println!("Found secrets to migrate:");
    for (app_id, app_name) in &ops_keys {
        println!(
            "  {} Ops key for {} ({})",
            style("•").dim(),
            style(app_name).cyan(),
            app_id
        );
    }
    println!();

    // Confirm
    if !Confirm::new()
        .with_prompt("Proceed with migration?")
        .default(true)
        .interact()?
    {
        println!("Migration cancelled.");
        return Ok(());
    }

    // Check auth (prompts for master password)
    let (auth, master_password) = load_auth_with_password()?;

    println!("{}", style("Unlocking vault...").dim());

    // Unlock pass vault
    let client = PassClient::new(auth.access_token.clone());
    let (mut vault, key, version) = client.unlock(&master_password).await?;

    let mut migrated_count = 0;

    // Migrate ops keys
    for (app_id, app_name) in &ops_keys {
        let note_name = format!("{}{}", CLI_OPS_PREFIX, app_id);

        // Check if already exists
        let exists = vault.items.iter().any(|item| {
            matches!(item, VaultItem::Note(n) if n.name == note_name && n.deleted_at.is_none())
        });

        if exists {
            println!(
                "  {} Ops key for {} already exists in vault, skipping",
                style("⚠").yellow(),
                app_name
            );
            continue;
        }

        if let Ok(private_key) = read_ops_key_from_keychain(app_id) {
            let note = create_note(&note_name, &private_key);
            vault.items.push(VaultItem::Note(note));
            migrated_count += 1;
            println!("  {} Ops key for {}", style("✓").green(), app_name);
        }
    }

    if migrated_count == 0 {
        println!("\n{} Nothing to migrate.", style("!").yellow());
        return Ok(());
    }

    // Save vault
    vault.last_modified = now_timestamp();
    println!("{}", style("Saving to vault...").dim());
    client.update_vault(&vault, &key, version).await?;

    println!(
        "\n{} Migrated {} secret(s) to Groo Pass",
        style("✓").green(),
        migrated_count
    );

    // Ask to delete from keychain
    println!();
    if Confirm::new()
        .with_prompt("Delete migrated ops keys from Keychain?")
        .default(false)
        .interact()?
    {
        let mut deleted = 0;

        for (app_id, _) in &ops_keys {
            if delete_ops_key_from_keychain(app_id).is_ok() {
                deleted += 1;
            }
        }

        println!(
            "{} Deleted {} key(s) from Keychain",
            style("✓").green(),
            deleted
        );
    }

    println!();
    println!(
        "{}",
        style("Migration complete! Your secrets are now stored in Groo Pass.").green()
    );

    Ok(())
}

fn read_ops_key_from_keychain(app_id: &str) -> Result<String> {
    let entry = Entry::new(SERVICE_NAME, &format!("ops-{}", app_id))?;
    Ok(entry.get_password()?)
}

fn delete_ops_key_from_keychain(app_id: &str) -> Result<()> {
    let entry = Entry::new(SERVICE_NAME, &format!("ops-{}", app_id))?;
    entry.delete_credential()?;
    Ok(())
}

/// Find ops keys by looking at ops.json in current project
fn find_ops_keys() -> Vec<(String, String)> {
    let mut keys = Vec::new();

    // Try to find project root and load ops config
    if let Ok(root) = find_project_root() {
        if let Ok(config) = OpsConfig::load(&root) {
            for link in config.services.values() {
                // Check if key exists in keychain
                if Entry::new(SERVICE_NAME, &format!("ops-{}", link.application_id))
                    .and_then(|e| e.get_password())
                    .is_ok()
                {
                    keys.push((link.application_id.clone(), link.application_name.clone()));
                }
            }
        }
    }

    // Deduplicate by app_id
    keys.sort_by(|a, b| a.0.cmp(&b.0));
    keys.dedup_by(|a, b| a.0 == b.0);

    keys
}

fn create_note(name: &str, content: &str) -> NoteItem {
    let now = now_timestamp();
    NoteItem {
        id: Uuid::new_v4().to_string(),
        name: name.to_string(),
        content: content.to_string(),
        folder_id: None,
        favorite: None,
        created_at: now,
        updated_at: now,
        deleted_at: None,
    }
}

fn now_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
}
