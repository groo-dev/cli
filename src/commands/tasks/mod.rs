pub mod add;
pub mod archive;
pub mod comment;
pub mod done;
pub mod list;
pub mod search;
pub mod show;
pub mod start;

use crate::discovery::find_project_root;
use crate::tasks::Project;

/// Get project from argument or detect from directory
pub fn resolve_project(projects: &[Project], project_arg: Option<&str>) -> Option<Project> {
    match project_arg {
        Some(name) => projects.iter().find(|p| p.name == name).cloned(),
        None => detect_project_from_dir(projects),
    }
}

/// Try to detect project from current directory by matching against user's projects
fn detect_project_from_dir(projects: &[Project]) -> Option<Project> {
    let root = find_project_root().ok()?;
    let dir_name = root.file_name()?.to_str()?;
    projects.iter().find(|p| p.name == dir_name).cloned()
}
