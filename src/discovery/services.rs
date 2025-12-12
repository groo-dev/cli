use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::process::Command;
use walkdir::WalkDir;

use super::ports::{detect_port, FrameworkType};

#[derive(Debug, Clone)]
pub struct Service {
    pub name: String,
    pub path: PathBuf,
    pub dev_command: String,
    #[allow(dead_code)]
    pub framework: FrameworkType,
    pub port: Option<u16>,
}

#[derive(Deserialize)]
struct PackageJson {
    scripts: Option<std::collections::HashMap<String, String>>,
}

pub fn find_git_root() -> Result<PathBuf> {
    let output = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .context("Failed to run git command")?;

    if !output.status.success() {
        anyhow::bail!("Not in a git repository");
    }

    let path = String::from_utf8(output.stdout)?
        .trim()
        .to_string();
    Ok(PathBuf::from(path))
}

pub fn get_project_name(git_root: &Path) -> String {
    git_root
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string()
}

pub fn discover_services(git_root: &Path) -> Result<Vec<Service>> {
    let mut services = Vec::new();

    for entry in WalkDir::new(git_root)
        .follow_links(true)
        .into_iter()
        .filter_entry(|e| !is_ignored(e.path()))
    {
        let entry = entry?;
        if entry.file_name() == "package.json" {
            let package_path = entry.path();
            let service_dir = package_path.parent().unwrap();

            // Skip root package.json
            if service_dir == git_root {
                continue;
            }

            if let Some(service) = parse_service(git_root, service_dir, package_path)? {
                services.push(service);
            }
        }
    }

    Ok(services)
}

fn is_ignored(path: &Path) -> bool {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    matches!(name, "node_modules" | ".git" | "dist" | "build" | ".next" | ".turbo")
}

fn parse_service(git_root: &Path, service_dir: &Path, package_path: &Path) -> Result<Option<Service>> {
    let content = std::fs::read_to_string(package_path)?;
    let package: PackageJson = serde_json::from_str(&content)?;

    let dev_command = match package.scripts {
        Some(scripts) => scripts.get("dev").cloned(),
        None => None,
    };

    let dev_command = match dev_command {
        Some(cmd) => cmd,
        None => return Ok(None),
    };

    // Skip orchestrator scripts (turbo, pnpm workspace, npm workspace, etc.)
    if is_orchestrator_script(&dev_command) {
        return Ok(None);
    }

    let framework = detect_framework(&dev_command, service_dir);
    let port = detect_port(&framework, &dev_command, service_dir);

    // Use relative path from git root as the service name
    let name = service_dir
        .strip_prefix(git_root)
        .ok()
        .and_then(|p| p.to_str())
        .map(|s| s.replace('/', ":"))
        .unwrap_or_else(|| {
            service_dir
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string()
        });

    Ok(Some(Service {
        name,
        path: service_dir.to_path_buf(),
        dev_command,
        framework,
        port,
    }))
}

fn is_orchestrator_script(dev_command: &str) -> bool {
    let orchestrators = [
        "turbo dev",
        "turbo run dev",
        "pnpm -r",
        "pnpm --filter",
        "pnpm run -r",
        "npm run --workspaces",
        "yarn workspaces",
        "lerna run",
    ];
    orchestrators.iter().any(|o| dev_command.contains(o))
}

fn detect_framework(dev_command: &str, service_dir: &Path) -> FrameworkType {
    // Check for wrangler
    if dev_command.contains("wrangler") {
        return FrameworkType::Wrangler;
    }

    // Check for wrangler config files
    if service_dir.join("wrangler.jsonc").exists() || service_dir.join("wrangler.toml").exists() {
        return FrameworkType::Wrangler;
    }

    // Check for Next.js
    if dev_command.contains("next") {
        return FrameworkType::NextJs;
    }

    // Check for Vite
    if dev_command.contains("vite") || service_dir.join("vite.config.ts").exists() || service_dir.join("vite.config.js").exists() {
        return FrameworkType::Vite;
    }

    FrameworkType::Unknown
}
