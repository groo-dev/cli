use anyhow::Result;
use console::style;
use dialoguer::Confirm;

use crate::auth::provider;
use crate::discovery::find_project_root;
use crate::tasks::{CreateProjectRequest, CreateTaskError, CreateTaskRequest, TasksClient};

pub async fn run(
    title: String,
    project: Option<String>,
    priority: Option<String>,
    tags: Option<Vec<String>>,
    description: Option<String>,
) -> Result<()> {
    let auth = provider::get_valid_auth().await?;
    let client = TasksClient::new(auth.access_token);

    // Determine project name from arg or current directory
    let project_name = match project {
        Some(name) => name,
        None => {
            // Try to detect from current directory
            let root = find_project_root()?;
            let dir_name = root
                .file_name()
                .and_then(|n| n.to_str())
                .ok_or_else(|| anyhow::anyhow!("Could not determine project name from directory"))?;
            dir_name.to_string()
        }
    };

    // Create the task request
    let request = CreateTaskRequest {
        project_name: project_name.clone(),
        title: title.clone(),
        description,
        status: None,
        priority,
        tags,
    };

    // Try to create the task
    match client.create_task(request.clone()).await {
        Ok(task) => {
            println!(
                "{} Created task in {}: {}",
                style("✓").green(),
                style(&project_name).cyan(),
                style(&task.title).bold()
            );
            println!("  ID: {}", style(&task.id).dim());
            Ok(())
        }
        Err(CreateTaskError::ProjectNotFound(name)) => {
            // Project doesn't exist, prompt to create it
            println!(
                "{} Project {} not found.",
                style("!").yellow(),
                style(&name).cyan()
            );

            let create = Confirm::new()
                .with_prompt(format!("Create project '{}'?", name))
                .default(true)
                .interact()?;

            if create {
                // Create the project
                let project = client
                    .create_project(CreateProjectRequest {
                        name: name.clone(),
                        description: None,
                    })
                    .await?;

                println!(
                    "{} Created project: {}",
                    style("✓").green(),
                    style(&project.name).cyan()
                );

                // Retry creating the task
                let task = client.create_task(request).await.map_err(|e| anyhow::anyhow!("{}", e))?;

                println!(
                    "{} Created task in {}: {}",
                    style("✓").green(),
                    style(&project_name).cyan(),
                    style(&task.title).bold()
                );
                println!("  ID: {}", style(&task.id).dim());
                Ok(())
            } else {
                println!("Cancelled.");
                Ok(())
            }
        }
        Err(e) => Err(anyhow::anyhow!("{}", e)),
    }
}
