use super::config::{AuthProvider, Feature, LaunchpadConfig, ProjectType, Resource};
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

    // Cross-cutting: auth providers that require D1
    for project in &config.projects {
        if let Some(provider) = project.auth_provider()
            && matches!(provider, AuthProvider::BetterAuth | AuthProvider::Simple)
            && !config.has_resource(&Resource::D1)
        {
            let label = match provider {
                AuthProvider::BetterAuth => "better-auth",
                AuthProvider::Simple => "simple",
                _ => unreachable!(),
            };
            errors.push(format!(
                "Auth provider '{}' requires D1 for storing user data. \
                 Add \"d1\" to the top-level resources array.",
                label
            ));
            break;
        }
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

fn validate_root(
    root_value: &str,
    root_path: &Path,
    config: &LaunchpadConfig,
    errors: &mut Vec<String>,
) {
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
        // Only error if the directory exists AND it's not from a previous launchpad run
        if root_path.exists() && !root_path.join(".launchpad-state.json").exists() {
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
    if config.has_hono_worker() && config.domain.is_none() {
        errors.push(
            "Missing 'domain': required when any worker has the 'hono' feature. \
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

    // ios/android must not have features
    if matches!(project.project_type, ProjectType::Ios | ProjectType::Android)
        && !project.features.is_empty()
    {
        errors.push(format!(
            "Project '{}': {} projects must not have features.",
            project.name,
            project.project_type.label()
        ));
        return;
    }

    // Validate each feature is valid for the project type
    let web_features = ["tailwind", "shadcn", "tanstack-router", "tanstack-query", "axios"];
    let worker_features = ["hono", "drizzle", "email"];

    for feature in &project.features {
        let feature_name = feature_type_name(feature);
        match project.project_type {
            ProjectType::Web => {
                if worker_features.contains(&feature_name) {
                    errors.push(format!(
                        "Project '{}': feature '{}' is not available on web projects. \
                         Move it to a worker project.",
                        project.name, feature_name
                    ));
                }
            }
            ProjectType::Worker if web_features.contains(&feature_name) => {
                errors.push(format!(
                    "Project '{}': feature '{}' is not available on worker projects. \
                     Move it to a web project.",
                    project.name, feature_name
                ));
            }
            _ => {}
        }
    }

    // auth(better-auth) requires drizzle on the same worker
    if let Some(AuthProvider::BetterAuth) = project.auth_provider()
        && project.project_type == ProjectType::Worker
        && !project.has_feature_type("drizzle")
    {
        errors.push(format!(
            "Project '{}': auth provider 'better-auth' requires the 'drizzle' feature. \
             Add {{ \"type\": \"drizzle\" }} to features.",
            project.name
        ));
    }

    // auth(simple) requires drizzle on the same worker
    if let Some(AuthProvider::Simple) = project.auth_provider()
        && project.project_type == ProjectType::Worker
        && !project.has_feature_type("drizzle")
    {
        errors.push(format!(
            "Project '{}': auth provider 'simple' requires the 'drizzle' feature. \
             Add {{ \"type\": \"drizzle\" }} to features.",
            project.name
        ));
    }

    // shadcn requires tailwind
    if project.has_feature_type("shadcn") && !project.has_feature_type("tailwind") {
        errors.push(format!(
            "Project '{}': 'shadcn' feature requires 'tailwind'. \
             Add {{ \"type\": \"tailwind\" }} to features.",
            project.name
        ));
    }
}

fn feature_type_name(feature: &Feature) -> &str {
    match feature {
        Feature::Tailwind => "tailwind",
        Feature::Shadcn => "shadcn",
        Feature::TanstackRouter => "tanstack-router",
        Feature::TanstackQuery => "tanstack-query",
        Feature::Axios => "axios",
        Feature::Hono => "hono",
        Feature::Drizzle => "drizzle",
        Feature::Auth { .. } => "auth",
        Feature::Email { .. } => "email",
    }
}

fn is_valid_dir_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
}
