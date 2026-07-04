use anyhow::Result;
use console::style;

use crate::auth::provider;
use crate::tasks::{TaskPriority, TaskStatus, TasksClient};

use super::resolve_task_id;

pub async fn run(id: String) -> Result<()> {
    let auth = provider::get_valid_auth().await?;
    let client = TasksClient::new(auth.access_token);

    // Resolve prefix to full ID
    let task_id = resolve_task_id(&client, &id).await?;
    let response = client.get_task(&task_id).await?;
    let task = response.task;
    let comments = response.comments.unwrap_or_default();
    let project = response.project;

    // Header
    let status_str = match task.status {
        TaskStatus::Backlog => style("BACKLOG").dim().to_string(),
        TaskStatus::Open => style("OPEN").white().to_string(),
        TaskStatus::InProgress => style("IN PROGRESS").yellow().to_string(),
        TaskStatus::Done => style("DONE").green().to_string(),
        TaskStatus::Archived => style("ARCHIVED").dim().to_string(),
    };

    let priority_str = match task.priority {
        TaskPriority::High => style("HIGH").red().to_string(),
        TaskPriority::Medium => style("MEDIUM").yellow().to_string(),
        TaskPriority::Low => style("LOW").dim().to_string(),
    };

    println!();
    println!("{}", style(&task.title).bold());
    println!();

    // Metadata
    println!("  {}  {}", style("Status:").dim(), status_str);
    println!("  {} {}", style("Priority:").dim(), priority_str);

    if let Some(p) = &project {
        println!("  {} {}", style("Project:").dim(), style(&p.name).cyan());
    }

    if let Some(tags) = &task.tags
        && !tags.is_empty() {
            let tags_str = tags
                .iter()
                .map(|t| style(t).magenta().to_string())
                .collect::<Vec<_>>()
                .join(", ");
            println!("  {}    {}", style("Tags:").dim(), tags_str);
        }

    println!("  {}      {}", style("ID:").dim(), style(&task.id).dim());
    println!("  {} {}", style("Created:").dim(), style(&task.created_at).dim());

    // Description
    if let Some(desc) = &task.description {
        println!();
        println!("{}", style("Description:").dim());
        println!("  {}", desc);
    }

    // Comments
    if !comments.is_empty() {
        println!();
        println!("{} ({})", style("Comments:").dim(), comments.len());
        for comment in &comments {
            let author = comment.author.as_deref().unwrap_or("anonymous");
            println!();
            println!("  {} {}", style(author).cyan(), style(&comment.created_at).dim());
            println!("  {}", comment.content);
        }
    }

    println!();

    Ok(())
}
