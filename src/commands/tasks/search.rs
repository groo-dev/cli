use anyhow::Result;
use console::style;

use crate::auth::storage::load_auth_with_password;
use crate::tasks::{TaskPriority, TaskStatus, TasksClient};

pub async fn run(query: String) -> Result<()> {
    let (auth, _) = load_auth_with_password()?;
    let client = TasksClient::new(auth.access_token);

    let tasks = client.search_tasks(&query).await?;

    if tasks.is_empty() {
        println!("No tasks found matching '{}'", query);
        return Ok(());
    }

    // Get projects for display
    let projects = client.list_projects().await?;
    let project_map: std::collections::HashMap<_, _> = projects
        .iter()
        .map(|p| (p.id.as_str(), p.name.as_str()))
        .collect();

    println!("Found {} tasks matching '{}':\n", tasks.len(), query);

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
            style(&task.id).dim()
        );

        // Show description snippet if available
        if let Some(desc) = &task.description {
            let snippet = if desc.len() > 60 {
                format!("{}...", &desc[..60])
            } else {
                desc.clone()
            };
            println!("  {}", style(snippet).dim());
        }
    }

    Ok(())
}
