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
use crate::tasks::{Project, Task, TasksClient};

/// Get project from argument or detect from directory
pub fn resolve_project(projects: &[Project], project_arg: Option<&str>) -> Option<Project> {
    match project_arg {
        Some(name) => projects.iter().find(|p| p.name == name).cloned(),
        None => detect_project_from_dir(projects),
    }
}

/// Try to detect project from current directory by matching against user's projects
fn detect_project_from_dir(projects: &[Project]) -> Option<Project> {
    let root = find_project_root().ok()?;
    let dir_name = root.file_name()?.to_str()?;
    projects.iter().find(|p| p.name == dir_name).cloned()
}

/// Resolve a task ID from a prefix - shows interactive selection if ambiguous
pub async fn resolve_task_id(client: &TasksClient, id_prefix: &str) -> Result<String> {
    // Fetch all tasks and filter by prefix
    let tasks = client.list_tasks(None, None, None, true).await?;
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
