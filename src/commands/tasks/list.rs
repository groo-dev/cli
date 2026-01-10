use anyhow::Result;
use console::style;

use crate::auth::storage::load_auth_with_password;
use crate::discovery::find_project_root;
use crate::tasks::{TaskPriority, TaskStatus, TasksClient};

pub async fn run(
    project: Option<String>,
    status: Option<String>,
    all: bool,
) -> Result<()> {
    let (auth, _) = load_auth_with_password()?;
    let client = TasksClient::new(auth.access_token);

    // Resolve project name from arg or directory
    let project_name = match project {
        Some(name) => Some(name),
        None => {
            find_project_root()
                .ok()
                .and_then(|r| r.file_name()?.to_str().map(String::from))
        }
    };

    // Fetch tasks by project name
    let tasks = client
        .list_tasks(project_name.as_deref(), status.as_deref(), None, all)
        .await?;

    if tasks.is_empty() {
        let filter_msg = match (&project_name, &status) {
            (Some(p), Some(s)) => format!(" for {} with status {}", p, s),
            (Some(p), None) => format!(" for {}", p),
            (None, Some(s)) => format!(" with status {}", s),
            (None, None) => String::new(),
        };
        println!("No tasks found{}", filter_msg);
        return Ok(());
    }

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

        println!(
            "{} {}{}  {}",
            status_icon,
            task.title,
            priority_badge,
            style(&task.id).dim()
        );
    }

    println!();
    println!("{} tasks", tasks.len());

    Ok(())
}
