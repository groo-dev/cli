#![allow(dead_code)]

use super::config::LaunchpadConfig;
use super::resources;
use super::state::LaunchpadState;
use super::ui::Ui;
use anyhow::Result;
use std::path::Path;

pub async fn clean_previous_run(
    ui: &Ui,
    config: &LaunchpadConfig,
    state: &LaunchpadState,
    root: &Path,
) -> Result<()> {
    ui.section("Cleaning previous run...");

    for project in &config.projects {
        let project_dir = root.join(&project.name);
        if project_dir.exists() {
            std::fs::remove_dir_all(&project_dir)?;
            ui.success(&format!("Removed {}/", project.name));
        }
    }

    if !state.created_resources.is_empty() {
        resources::delete_resources(ui, state, root).await?;
    }

    let files_to_clean = [
        "CLAUDE.md",
        "README.md",
        "TODO.md",
        ".gitignore",
        ".claude/settings.local.json",
    ];
    for file in &files_to_clean {
        let path = root.join(file);
        if path.exists() {
            std::fs::remove_file(&path)?;
        }
    }

    let workflows_dir = root.join(".github/workflows");
    if workflows_dir.exists() {
        std::fs::remove_dir_all(&workflows_dir)?;
    }

    let git_dir = root.join(".git");
    if git_dir.exists() {
        std::fs::remove_dir_all(&git_dir)?;
    }

    state.delete()?;

    ui.newline();
    ui.section("Starting fresh...");
    ui.newline();

    Ok(())
}
