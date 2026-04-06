use super::config::{LaunchpadConfig, ProjectType};
use std::collections::HashSet;
use std::path::Path;

pub fn validate(config: &LaunchpadConfig, root: &Path) -> Vec<String> {
    let mut errors = Vec::new();

    validate_name(&config.name, &mut errors);
    validate_root(&config.root, root, config, &mut errors);
    validate_projects(&config.projects, &mut errors);
    validate_domain(config, &mut errors);

    for project in &config.projects {
        validate_project(project, &mut errors);
    }

    errors
}

fn validate_name(name: &str, errors: &mut Vec<String>) {
    if name.is_empty() {
        errors.push("'name' must not be empty.".to_string());
        return;
    }
    if !is_valid_dir_name(name) {
        errors.push(format!(
            "'name' value '{}' is not a valid directory name. \
             Use only alphanumeric characters, hyphens, and underscores.",
            name
        ));
    }
}

fn validate_root(root_value: &str, root_path: &Path, config: &LaunchpadConfig, errors: &mut Vec<String>) {
    if root_value == "." {
        for project in &config.projects {
            let project_dir = root_path.join(&project.name);
            if project_dir.exists() {
                errors.push(format!(
                    "Directory '{}' already exists in current directory. \
                     Remove it or use a different project name.",
                    project.name
                ));
            }
        }
    } else {
        if !is_valid_dir_name(root_value) {
            errors.push(format!(
                "'root' value '{}' is not a valid directory name. \
                 Use only alphanumeric characters, hyphens, and underscores.",
                root_value
            ));
        }
        if root_path.exists() {
            errors.push(format!(
                "Directory '{}' already exists. Remove it or choose a different name.",
                root_value
            ));
        }
    }
}

fn validate_projects(projects: &[super::config::ProjectConfig], errors: &mut Vec<String>) {
    if projects.is_empty() {
        errors.push("'projects' must contain at least one project.".to_string());
        return;
    }

    let mut seen = HashSet::new();
    for project in projects {
        if !seen.insert(&project.name) {
            errors.push(format!(
                "Duplicate project name '{}'. Each project must have a unique name.",
                project.name
            ));
        }
    }
}

fn validate_domain(config: &LaunchpadConfig, errors: &mut Vec<String>) {
    if config.has_api_worker() && config.domain.is_none() {
        errors.push(
            "Missing 'domain': required when any project is an API worker. \
             Add a domain like \"myapp.groo.bot\"."
                .to_string(),
        );
    }
}

fn validate_project(project: &super::config::ProjectConfig, errors: &mut Vec<String>) {
    if project.name.is_empty() {
        errors.push("Project name must not be empty.".to_string());
        return;
    }
    if !is_valid_dir_name(&project.name) {
        errors.push(format!(
            "Project '{}': name is not a valid directory name. \
             Use only alphanumeric characters, hyphens, and underscores.",
            project.name
        ));
    }

    match project.project_type {
        ProjectType::Web => {
            if !project.resources.is_empty() {
                errors.push(format!(
                    "Project '{}': web projects don't have Cloudflare resource bindings. \
                     Remove 'resources' or change type to 'api-worker'.",
                    project.name
                ));
            }
            if project.email.is_some() {
                errors.push(format!(
                    "Project '{}': web projects don't have email integration. \
                     Remove 'email' or move it to an API worker.",
                    project.name
                ));
            }
        }
        ProjectType::LightweightWorker => {
            if project.auth.is_some() {
                errors.push(format!(
                    "Project '{}': lightweight workers don't have auth. \
                     Remove 'auth' or change type to 'api-worker'.",
                    project.name
                ));
            }
            if project.email.is_some() {
                errors.push(format!(
                    "Project '{}': lightweight workers don't have email integration. \
                     Remove 'email' or change type to 'api-worker'.",
                    project.name
                ));
            }
        }
        ProjectType::Ios | ProjectType::Android => {
            if !project.resources.is_empty() {
                errors.push(format!(
                    "Project '{}': {} projects don't have Cloudflare resources. \
                     Remove 'resources'.",
                    project.name,
                    project.project_type.label()
                ));
            }
            if project.auth.is_some() {
                errors.push(format!(
                    "Project '{}': {} projects don't have auth configured via launchpad. \
                     Remove 'auth'.",
                    project.name,
                    project.project_type.label()
                ));
            }
            if project.email.is_some() {
                errors.push(format!(
                    "Project '{}': {} projects don't have email integration. \
                     Remove 'email'.",
                    project.name,
                    project.project_type.label()
                ));
            }
        }
        ProjectType::ApiWorker => {}
    }
}

fn is_valid_dir_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
}
