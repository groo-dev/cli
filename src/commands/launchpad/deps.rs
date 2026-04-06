use super::config::{AuthProvider, EmailProvider, ProjectConfig, ProjectType};
use super::ui::Ui;
use anyhow::Result;
use std::path::Path;

pub async fn install_deps(ui: &Ui, project: &ProjectConfig, project_dir: &Path) -> Result<()> {
    match project.project_type {
        ProjectType::Web => install_web_deps(ui, project, project_dir).await,
        ProjectType::ApiWorker => install_api_worker_deps(ui, project, project_dir).await,
        ProjectType::LightweightWorker => install_lightweight_worker_deps(ui, project_dir).await,
        ProjectType::Ios | ProjectType::Android => Ok(()),
    }
}

async fn install_web_deps(ui: &Ui, project: &ProjectConfig, dir: &Path) -> Result<()> {
    let mut deps = vec![
        "@tanstack/react-router",
        "@tanstack/react-query",
        "axios",
        "date-fns",
        "clsx",
        "tailwind-merge",
        "class-variance-authority",
        "lucide-react",
    ];

    match &project.auth {
        Some(AuthProvider::Clerk) => {
            deps.push("@clerk/clerk-react");
            deps.push("@clerk/themes");
        }
        Some(AuthProvider::BetterAuth) => {
            deps.push("better-auth");
        }
        _ => {}
    }

    let dep_count = deps.len();
    ui.run_command(
        &format!("Installed {} packages", dep_count),
        &format!("npm install {}", deps.join(" ")),
        dir,
    )
    .await?;

    let dev_deps = [
        "tailwindcss",
        "@tailwindcss/vite",
        "typescript",
        "@vitejs/plugin-react",
        "eslint",
        "wrangler",
    ];

    ui.run_command(
        &format!("Installed {} dev packages", dev_deps.len()),
        &format!("npm install -D {}", dev_deps.join(" ")),
        dir,
    )
    .await?;

    Ok(())
}

async fn install_api_worker_deps(ui: &Ui, project: &ProjectConfig, dir: &Path) -> Result<()> {
    let mut deps = vec!["hono", "drizzle-orm"];

    if let Some(AuthProvider::Clerk) = &project.auth {
        deps.push("@clerk/backend")
    }

    if let Some(EmailProvider::Resend) = &project.email {
        deps.push("resend")
    }

    let dep_count = deps.len();
    ui.run_command(
        &format!("Installed {} packages", dep_count),
        &format!("npm install {}", deps.join(" ")),
        dir,
    )
    .await?;

    let dev_deps = ["drizzle-kit", "wrangler", "@types/node"];

    ui.run_command(
        &format!("Installed {} dev packages", dev_deps.len()),
        &format!("npm install -D {}", dev_deps.join(" ")),
        dir,
    )
    .await?;

    Ok(())
}

async fn install_lightweight_worker_deps(ui: &Ui, dir: &Path) -> Result<()> {
    ui.run_command(
        "Installed dev packages",
        "npm install -D wrangler @types/node",
        dir,
    )
    .await?;

    Ok(())
}
