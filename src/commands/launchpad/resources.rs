use super::config::{LaunchpadConfig, ProjectType, Resource};
use super::state::LaunchpadState;
use super::ui::Ui;
use anyhow::Result;
use std::path::Path;

pub async fn create_resources(
    ui: &Ui,
    config: &LaunchpadConfig,
    state: &mut LaunchpadState,
    root: &Path,
) -> Result<()> {
    for project in &config.projects {
        if project.project_type == ProjectType::Web {
            let pages_name = format!("{}-web", config.name);
            ui.run_command(
                &format!("Created Pages project \"{}\"", pages_name),
                &format!("wrangler pages project create {}", pages_name),
                root,
            )
            .await?;
            state.add_resource("pages", &pages_name, "");
            state.save()?;
        }
    }

    // Create top-level resources (shared across all workers)
    for resource in &config.resources {
        match resource {
            Resource::D1 => {
                let name = format!("{}-d1", config.name);
                let output = ui
                    .run_command(
                        &format!("Created D1 database \"{}\"", name),
                        &format!("wrangler d1 create {}", name),
                        root,
                    )
                    .await?;
                let id = parse_d1_id(&output).unwrap_or_default();
                state.add_resource("d1", &name, &id);
                state.save()?;
            }
            Resource::R2 => {
                let name = format!("{}-r2", config.name);
                ui.run_command(
                    &format!("Created R2 bucket \"{}\"", name),
                    &format!("wrangler r2 bucket create {}", name),
                    root,
                )
                .await?;
                state.add_resource("r2", &name, "");
                state.save()?;
            }
            Resource::Kv => {
                let name = format!("{}-kv", config.name);
                let output = ui
                    .run_command(
                        &format!("Created KV namespace \"{}\"", name),
                        &format!("wrangler kv namespace create {}", name),
                        root,
                    )
                    .await?;
                let id = parse_kv_id(&output).unwrap_or_default();
                state.add_resource("kv", &name, &id);
                state.save()?;
            }
            Resource::Queues => {
                let name = format!("{}-queue", config.name);
                ui.run_command(
                    &format!("Created Queue \"{}\"", name),
                    &format!("wrangler queues create {}", name),
                    root,
                )
                .await?;
                state.add_resource("queues", &name, "");
                state.save()?;
            }
            Resource::AiGateway => {}
        }
    }

    Ok(())
}

fn parse_d1_id(output: &str) -> Option<String> {
    for line in output.lines() {
        if let Some(id_part) = line.strip_prefix("database_id = ") {
            return Some(id_part.trim().trim_matches('"').to_string());
        }
        if line.contains("database_id") && line.contains('|') {
            let parts: Vec<&str> = line.split('|').collect();
            if parts.len() >= 3 {
                return Some(parts[2].trim().to_string());
            }
        }
    }
    None
}

fn parse_kv_id(output: &str) -> Option<String> {
    for line in output.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("id = ") || trimmed.starts_with("\"id\": ") {
            return Some(
                trimmed
                    .split('=')
                    .next_back()
                    .or_else(|| trimmed.split(':').next_back())
                    .unwrap_or("")
                    .trim()
                    .trim_matches('"')
                    .trim_matches(',')
                    .to_string(),
            );
        }
    }
    None
}

pub async fn delete_resources(ui: &Ui, state: &LaunchpadState, root: &Path) -> Result<()> {
    for resource in &state.created_resources {
        match resource.resource_type.as_str() {
            "d1" => {
                let _ = ui
                    .run_command(
                        &format!("Deleted D1 database \"{}\"", resource.name),
                        &format!("wrangler d1 delete {} --yes", resource.name),
                        root,
                    )
                    .await;
            }
            "r2" => {
                let _ = ui
                    .run_command(
                        &format!("Deleted R2 bucket \"{}\"", resource.name),
                        &format!("wrangler r2 bucket delete {}", resource.name),
                        root,
                    )
                    .await;
            }
            "kv" => {
                if !resource.id.is_empty() {
                    let _ = ui
                        .run_command(
                            &format!("Deleted KV namespace \"{}\"", resource.name),
                            &format!("wrangler kv namespace delete --namespace-id {}", resource.id),
                            root,
                        )
                        .await;
                }
            }
            "queues" => {
                let _ = ui
                    .run_command(
                        &format!("Deleted Queue \"{}\"", resource.name),
                        &format!("wrangler queues delete {}", resource.name),
                        root,
                    )
                    .await;
            }
            "pages" => {
                let _ = ui
                    .run_command(
                        &format!("Deleted Pages project \"{}\"", resource.name),
                        &format!("wrangler pages project delete {} --yes", resource.name),
                        root,
                    )
                    .await;
            }
            _ => {}
        }
    }
    Ok(())
}
