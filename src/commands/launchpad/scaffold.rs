use super::config::ProjectType;
use super::ui::Ui;
use anyhow::Result;
use std::path::Path;

pub async fn scaffold_project(
    ui: &Ui,
    project_name: &str,
    project_type: &ProjectType,
    root: &Path,
) -> Result<()> {
    match project_type {
        ProjectType::Web => {
            ui.run_command(
                "Scaffolded with Vite + React + TypeScript",
                &format!("npm create vite@latest {} -- --template react-ts", project_name),
                root,
            )
            .await?;
        }
        ProjectType::Worker => {
            ui.run_command(
                "Scaffolded with Cloudflare Worker",
                &format!(
                    "npm create cloudflare@latest {} -- --type hello-world --lang ts --no-git --no-deploy --no-agents -y",
                    project_name
                ),
                root,
            )
            .await?;
        }
        ProjectType::Ios => {
            ui.success(&format!(
                "iOS project '{}' — create via Xcode: File → New → Project → App (SwiftUI)",
                project_name
            ));
        }
        ProjectType::Android => {
            ui.success(&format!(
                "Android project '{}' — create via Android Studio: New Project → Empty Activity (Kotlin)",
                project_name
            ));
        }
    }
    Ok(())
}
