use anyhow::{anyhow, Result};
use console::style;
use std::io::Read;
use std::path::PathBuf;

use crate::auth::storage::load_auth_with_password;
use crate::pad::client::PadClient;

pub async fn run(text: Option<String>, files: Vec<PathBuf>) -> Result<()> {
    // Check auth (prompts for master password)
    let (auth, _master_password) = load_auth_with_password()?;

    // Get text from argument or stdin
    let text_content = get_text_content(text)?;

    // Validate we have something to add
    if text_content.is_none() && files.is_empty() {
        return Err(anyhow!("Provide text, --file, or both"));
    }

    // Resolve file paths (globs, folders)
    let resolved_files = resolve_files(&files)?;

    // Prompt for pad encryption password
    let password = rpassword::prompt_password("Pad encryption password: ")?;

    let client = PadClient::new(auth.access_token);

    // If we have files, upload them first
    let file_attachments = if !resolved_files.is_empty() {
        println!("Uploading {} file(s)...", resolved_files.len());

        // Get encryption key
        let key = client.get_encryption_salt(&password).await?;

        let mut attachments = Vec::new();
        for path in &resolved_files {
            let file_name = path
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "unknown".to_string());

            print!("  {} ", file_name);

            let data = std::fs::read(path)?;
            let mime_type = mime_guess::from_path(path)
                .first_or_octet_stream()
                .to_string();

            let attachment = client.upload_file(&data, &file_name, &mime_type, &key).await?;
            attachments.push(attachment);

            println!("{}", style("✓").green());
        }
        attachments
    } else {
        Vec::new()
    };

    // Add list item
    print!("Adding to list... ");
    client
        .add_list_item(text_content.as_deref(), file_attachments, &password)
        .await?;
    println!("{}", style("✓").green());

    // Summary
    let mut parts = Vec::new();
    if text_content.is_some() {
        parts.push("text");
    }
    if !resolved_files.is_empty() {
        parts.push(if resolved_files.len() == 1 { "1 file" } else { "files" });
    }
    println!(
        "\n{} Added {} to your pad",
        style("✓").green(),
        parts.join(" + ")
    );

    Ok(())
}

fn get_text_content(text_arg: Option<String>) -> Result<Option<String>> {
    // If text provided as argument, use it
    if let Some(t) = text_arg
        && !t.is_empty() {
            return Ok(Some(t));
        }

    // Check if stdin has data (piped input)
    if !atty::is(atty::Stream::Stdin) {
        let mut buffer = String::new();
        std::io::stdin().read_to_string(&mut buffer)?;
        if !buffer.is_empty() {
            return Ok(Some(buffer));
        }
    }

    Ok(None)
}

fn resolve_files(paths: &[PathBuf]) -> Result<Vec<PathBuf>> {
    let mut result = Vec::new();

    for path in paths {
        let path_str = path.to_string_lossy();

        // Check if it's a glob pattern
        if path_str.contains('*') || path_str.contains('?') {
            for entry in glob::glob(&path_str)? {
                let entry = entry?;
                if entry.is_file() {
                    result.push(entry);
                }
            }
        } else if path.is_dir() {
            // Walk directory
            for entry in walkdir::WalkDir::new(path)
                .into_iter()
                .filter_map(|e| e.ok())
            {
                if entry.file_type().is_file() {
                    result.push(entry.path().to_path_buf());
                }
            }
        } else if path.is_file() {
            result.push(path.clone());
        } else {
            return Err(anyhow!("File not found: {}", path.display()));
        }
    }

    Ok(result)
}
