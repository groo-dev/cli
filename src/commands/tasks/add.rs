use anyhow::Result;
use console::style;
use dialoguer::FuzzySelect;

use crate::auth::storage::load_auth_with_password;
use crate::tasks::{CreateTaskRequest, TasksClient};

use super::resolve_project;

pub async fn run(
    title: String,
    project: Option<String>,
    priority: Option<String>,
    tags: Option<Vec<String>>,
    description: Option<String>,
) -> Result<()> {
    let (auth, _) = load_auth_with_password()?;
    let client = TasksClient::new(auth.access_token);

    // Get projects
    let projects = client.list_projects().await?;

    if projects.is_empty() {
        println!(
            "{} No projects found. Create projects at {}",
            style("!").yellow(),
            style("https://tasks.groo.dev").cyan()
        );
        return Ok(());
    }

    // Resolve project from arg or directory
    let resolved = resolve_project(&client, project.clone()).await?;

    let selected_project = match resolved {
        Some(p) => p,
        None => {
            // Interactive selection
            let names: Vec<&str> = projects.iter().map(|p| p.name.as_str()).collect();
            let selection = FuzzySelect::new()
                .with_prompt("Select project")
                .items(&names)
                .interact()?;
            projects[selection].clone()
        }
    };

    // Create the task
    let request = CreateTaskRequest {
        project_id: selected_project.id.clone(),
        title: title.clone(),
        description,
        status: None,
        priority,
        tags,
    };

    let task = client.create_task(request).await?;

    println!(
        "{} Created task in {}: {}",
        style("✓").green(),
        style(&selected_project.name).cyan(),
        style(&task.title).bold()
    );
    println!("  ID: {}", style(&task.id).dim());

    Ok(())
}
