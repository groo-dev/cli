pub mod add;
pub mod archive;
pub mod comment;
pub mod done;
pub mod list;
pub mod search;
pub mod show;
pub mod start;

use anyhow::{bail, Result};
use console::style;
use dialoguer::FuzzySelect;

use crate::discovery::find_project_root;
use crate::tasks::{Task, TasksClient};

/// Resolve a task ID from a prefix - shows interactive selection if ambiguous
pub async fn resolve_task_id(client: &TasksClient, id_prefix: &str) -> Result<String> {
    // Get project name from current directory
    let project_name = detect_project_name();

    // Fetch tasks for this project and filter by prefix
    let tasks = client.list_tasks(project_name.as_deref(), None, None, true).await?;
    let matches: Vec<&Task> = tasks
        .iter()
        .filter(|t| t.id.starts_with(id_prefix))
        .collect();

    match matches.len() {
        0 => bail!("No task found matching '{}'", id_prefix),
        1 => Ok(matches[0].id.clone()),
        _ => {
            // Show interactive selection
            println!(
                "{} Multiple tasks match '{}'. Select one:",
                style("?").yellow(),
                id_prefix
            );

            let items: Vec<String> = matches
                .iter()
                .map(|t| format!("{} - {}", &t.id[..8], t.title))
                .collect();

            let selection = FuzzySelect::new()
                .with_prompt("Task")
                .items(&items)
                .interact()?;

            Ok(matches[selection].id.clone())
        }
    }
}

/// Detect project name from current directory
fn detect_project_name() -> Option<String> {
    let root = find_project_root().ok()?;
    let dir_name = root.file_name()?.to_str()?;
    Some(dir_name.to_string())
}
