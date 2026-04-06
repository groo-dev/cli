use super::config::{AuthProvider, EmailProvider, Feature, ProjectConfig, ProjectType};
use super::ui::Ui;
use anyhow::Result;
use std::path::Path;

pub async fn install_deps(ui: &Ui, project: &ProjectConfig, project_dir: &Path) -> Result<()> {
    match project.project_type {
        ProjectType::Web => install_web_deps(ui, project, project_dir).await,
        ProjectType::Worker => install_worker_deps(ui, project, project_dir).await,
        ProjectType::Ios | ProjectType::Android => Ok(()),
    }
}

async fn install_web_deps(ui: &Ui, project: &ProjectConfig, dir: &Path) -> Result<()> {
    let mut deps: Vec<&str> = Vec::new();
    let mut dev_deps: Vec<&str> = vec!["typescript", "@vitejs/plugin-react", "eslint", "wrangler"];

    for feature in &project.features {
        match feature {
            Feature::Tailwind => {
                dev_deps.push("tailwindcss");
                dev_deps.push("@tailwindcss/vite");
            }
            Feature::Shadcn => {
                // shadcn init handles clsx, tailwind-merge, class-variance-authority
                deps.push("lucide-react");
            }
            Feature::TanstackRouter => {
                deps.push("@tanstack/react-router");
                dev_deps.push("@tanstack/router-plugin");
            }
            Feature::TanstackQuery => {
                deps.push("@tanstack/react-query");
            }
            Feature::Axios => {
                deps.push("axios");
            }
            Feature::Auth {
                provider: AuthProvider::Clerk,
            } => {
                deps.push("@clerk/clerk-react");
                deps.push("@clerk/themes");
                deps.push("@hono/clerk-auth");
            }
            Feature::Auth {
                provider: AuthProvider::BetterAuth,
            } => {
                deps.push("better-auth");
            }
            Feature::Auth { .. } => {}
            _ => {}
        }
    }

    if !deps.is_empty() {
        let count = deps.len();
        ui.run_command(
            &format!("Installed {} packages", count),
            &format!("npm install {}", deps.join(" ")),
            dir,
        )
        .await?;
    }

    ui.run_command(
        &format!("Installed {} dev packages", dev_deps.len()),
        &format!("npm install -D {}", dev_deps.join(" ")),
        dir,
    )
    .await?;

    Ok(())
}

async fn install_worker_deps(ui: &Ui, project: &ProjectConfig, dir: &Path) -> Result<()> {
    let mut deps: Vec<&str> = Vec::new();
    let mut dev_deps: Vec<&str> = vec!["wrangler", "@types/node"];

    for feature in &project.features {
        match feature {
            Feature::Hono => {
                deps.push("hono");
            }
            Feature::Drizzle => {
                deps.push("drizzle-orm");
                dev_deps.push("drizzle-kit");
            }
            Feature::Auth {
                provider: AuthProvider::Clerk,
            } => {
                deps.push("@hono/clerk-auth");
                deps.push("@clerk/backend");
            }
            Feature::Auth {
                provider: AuthProvider::BetterAuth,
            } => {
                deps.push("better-auth");
            }
            Feature::Email {
                provider: EmailProvider::Resend,
            } => {
                deps.push("resend");
            }
            _ => {}
        }
    }

    if !deps.is_empty() {
        let count = deps.len();
        ui.run_command(
            &format!("Installed {} packages", count),
            &format!("npm install {}", deps.join(" ")),
            dir,
        )
        .await?;
    }

    ui.run_command(
        &format!("Installed {} dev packages", dev_deps.len()),
        &format!("npm install -D {}", dev_deps.join(" ")),
        dir,
    )
    .await?;

    Ok(())
}
