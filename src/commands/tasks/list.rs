use anyhow::Result;
use console::style;

use crate::auth::storage::load_auth_with_password;
use crate::tasks::{TaskPriority, TaskStatus, TasksClient};

use super::resolve_project;

pub async fn run(
    project: Option<String>,
    status: Option<String>,
    all: bool,
) -> Result<()> {
    let (auth, _) = load_auth_with_password()?;
    let client = TasksClient::new(auth.access_token);

    // Get projects for display and project resolution
    let projects = client.list_projects().await?;

    // Resolve project from arg or directory
    let resolved = resolve_project(&projects, project.as_deref());
    let project_id = resolved.as_ref().map(|p| p.id.as_str());

    // Fetch tasks
    let tasks = client
        .list_tasks(project_id, status.as_deref(), None, all)
        .await?;

    if tasks.is_empty() {
        let filter_msg = match (&resolved, &status) {
            (Some(p), Some(s)) => format!(" for {} with status {}", p.name, s),
            (Some(p), None) => format!(" for {}", p.name),
            (None, Some(s)) => format!(" with status {}", s),
            (None, None) => String::new(),
        };
        println!("No tasks found{}", filter_msg);
        return Ok(());
    }

    // Build project lookup map
    let project_map: std::collections::HashMap<_, _> = projects
        .iter()
        .map(|p| (p.id.as_str(), p.name.as_str()))
        .collect();

    // Print tasks
    for task in &tasks {
        let status_icon = match task.status {
            TaskStatus::Backlog => style("○").dim(),
            TaskStatus::Open => style("○").white(),
            TaskStatus::InProgress => style("●").yellow(),
            TaskStatus::Done => style("✓").green(),
            TaskStatus::Archived => style("□").dim(),
        };

        let priority_badge = match task.priority {
            TaskPriority::High => format!(" {}", style("!high").red()),
            TaskPriority::Medium => String::new(),
            TaskPriority::Low => format!(" {}", style("low").dim()),
        };

        let project_display = project_map
            .get(task.project_id.as_str())
            .unwrap_or(&"?");

        println!(
            "{} [{}] {}{}  {}",
            status_icon,
            style(project_display).cyan(),
            task.title,
            priority_badge,
            style(&task.id[..8]).dim()
        );
    }

    println!();
    println!("{} tasks", tasks.len());

    Ok(())
}
