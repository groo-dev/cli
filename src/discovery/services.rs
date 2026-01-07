use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::{Path, PathBuf};
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

pub fn find_project_root() -> Result<PathBuf> {
    let current_dir = std::env::current_dir().context("Failed to get current directory")?;

    // Walk up looking for .groo or .git
    let mut dir = current_dir.as_path();
    loop {
        // Check for .groo first
        if dir.join(".groo").is_dir() {
            return Ok(dir.to_path_buf());
        }
        // Fall back to .git
        if dir.join(".git").exists() {
            return Ok(dir.to_path_buf());
        }

        // Move to parent directory
        match dir.parent() {
            Some(parent) => dir = parent,
            None => break,
        }
    }

    anyhow::bail!("Not in a project (no .groo or .git directory found)")
}

pub fn get_project_name(git_root: &Path) -> String {
    git_root
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string()
}

pub fn discover_services(git_root: &Path) -> Result<Vec<Service>> {
    discover_services_by_script(git_root, "dev")
}

pub fn discover_services_by_script(git_root: &Path, script: &str) -> Result<Vec<Service>> {
    let mut services = Vec::new();

    for entry in WalkDir::new(git_root)
        .follow_links(true)
        .into_iter()
        .filter_entry(|e| !is_ignored(e.path()))
    {
        let entry = entry?;
        let file_name = entry.file_name().to_str().unwrap_or("");
        let service_dir = entry.path().parent().unwrap();
        let is_root = service_dir == git_root;

        let service = match file_name {
            // Skip root package.json (usually orchestrator)
            "package.json" if !is_root => parse_npm_service(git_root, service_dir, entry.path(), script)?,
            // Makefile support (at any level including root)
            "Makefile" => parse_make_service(git_root, service_dir, entry.path(), script)?,
            _ => None,
        };

        if let Some(s) = service {
            services.push(s);
        }
    }

    Ok(services)
}

fn is_ignored(path: &Path) -> bool {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    matches!(name, "node_modules" | ".git" | "dist" | "build" | ".next" | ".turbo")
}

fn get_service_name(git_root: &Path, service_dir: &Path) -> String {
    if service_dir == git_root {
        // Use project directory name for root
        return git_root
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("root")
            .to_string();
    }

    service_dir
        .strip_prefix(git_root)
        .ok()
        .and_then(|p| p.to_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.replace('/', ":"))
        .unwrap_or_else(|| {
            service_dir
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string()
        })
}

fn parse_make_service(git_root: &Path, service_dir: &Path, makefile_path: &Path, script: &str) -> Result<Option<Service>> {
    // Check if Makefile has the target we're looking for
    let content = std::fs::read_to_string(makefile_path)?;

    // Look for target definition (e.g., "build:" or "lint:")
    let target_pattern = format!("{}:", script);
    if !content.lines().any(|line| line.starts_with(&target_pattern)) {
        return Ok(None);
    }

    let name = get_service_name(git_root, service_dir);
    Ok(Some(Service {
        name,
        path: service_dir.to_path_buf(),
        dev_command: format!("make {}", script),
        framework: FrameworkType::Unknown,
        port: None,
    }))
}

fn parse_npm_service(git_root: &Path, service_dir: &Path, package_path: &Path, script: &str) -> Result<Option<Service>> {
    let content = std::fs::read_to_string(package_path)?;
    let package: PackageJson = serde_json::from_str(&content)?;

    let script_command = match package.scripts {
        Some(scripts) => scripts.get(script).cloned(),
        None => None,
    };

    let script_command = match script_command {
        Some(cmd) => cmd,
        None => return Ok(None),
    };

    // Skip orchestrator scripts (turbo, pnpm workspace, npm workspace, etc.)
    if is_orchestrator_script(&script_command) {
        return Ok(None);
    }

    let framework = detect_framework(&script_command, service_dir);
    let port = detect_port(&framework, &script_command, service_dir);
    let name = get_service_name(git_root, service_dir);

    Ok(Some(Service {
        name,
        path: service_dir.to_path_buf(),
        dev_command: script_command,
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
    // Prioritize dev command detection over config files
    // (a project may have wrangler.toml for deployment but use vite for dev)

    // Check for Vite in command first
    if dev_command.contains("vite") {
        return FrameworkType::Vite;
    }

    // Check for Next.js
    if dev_command.contains("next") {
        return FrameworkType::NextJs;
    }

    // Check for wrangler in command
    if dev_command.contains("wrangler") {
        return FrameworkType::Wrangler;
    }

    // Fall back to config file detection
    if service_dir.join("vite.config.ts").exists()
        || service_dir.join("vite.config.js").exists()
        || service_dir.join("vite.config.mts").exists()
        || service_dir.join("vite.config.mjs").exists()
    {
        return FrameworkType::Vite;
    }

    if service_dir.join("wrangler.jsonc").exists() || service_dir.join("wrangler.toml").exists() {
        return FrameworkType::Wrangler;
    }

    FrameworkType::Unknown
}
