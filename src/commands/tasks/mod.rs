pub mod add;
pub mod archive;
pub mod comment;
pub mod done;
pub mod list;
pub mod search;
pub mod show;
pub mod start;

use anyhow::Result;
use crate::discovery::find_project_root;
use crate::tasks::{Project, TasksClient};

/// Try to detect project from current directory by matching against user's projects
pub async fn detect_project(client: &TasksClient) -> Result<Option<Project>> {
    let root = match find_project_root() {
        Ok(r) => r,
        Err(_) => return Ok(None),
    };

    let dir_name = match root.file_name().and_then(|n| n.to_str()) {
        Some(n) => n,
        None => return Ok(None),
    };

    // Fetch user's projects and find one matching the directory name
    let projects = client.list_projects().await?;
    Ok(projects.into_iter().find(|p| p.name == dir_name))
}

/// Get project from argument or detect from directory
pub async fn resolve_project(client: &TasksClient, project_arg: Option<String>) -> Result<Option<Project>> {
    match project_arg {
        Some(name) => {
            let projects = client.list_projects().await?;
            Ok(projects.into_iter().find(|p| p.name == name))
        }
        None => detect_project(client).await,
    }
}
