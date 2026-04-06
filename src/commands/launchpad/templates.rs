use super::config::{
    AuthProvider, EmailProvider, LaunchpadConfig, ProjectConfig, ProjectType, Resource,
};
use anyhow::Result;
use std::path::Path;
use tera::{Context, Tera};

/// All templates embedded at compile time
const WRANGLER_TEMPLATE: &str = include_str!("../../../templates/launchpad/wrangler.jsonc.tera");
const VITE_CONFIG_TEMPLATE: &str = include_str!("../../../templates/launchpad/vite.config.ts.tera");
const DRIZZLE_CONFIG_TEMPLATE: &str =
    include_str!("../../../templates/launchpad/drizzle.config.ts.tera");
const HONO_ENTRY_TEMPLATE: &str = include_str!("../../../templates/launchpad/hono-entry.ts.tera");
const AXIOS_CLIENT_TEMPLATE: &str =
    include_str!("../../../templates/launchpad/axios-client.ts.tera");
const CONFIG_WORKER_TEMPLATE: &str =
    include_str!("../../../templates/launchpad/config-worker.ts.tera");
const CONFIG_WEB_TEMPLATE: &str = include_str!("../../../templates/launchpad/config-web.ts.tera");
const SCHEMA_TEMPLATE: &str = include_str!("../../../templates/launchpad/schema.ts.tera");
const ENV_EXAMPLE_TEMPLATE: &str = include_str!("../../../templates/launchpad/env.example.tera");
const DEV_VARS_EXAMPLE_TEMPLATE: &str =
    include_str!("../../../templates/launchpad/dev.vars.example.tera");
const DEPLOY_WORKER_TEMPLATE: &str =
    include_str!("../../../templates/launchpad/deploy-worker.yml.tera");
const DEPLOY_WEB_TEMPLATE: &str = include_str!("../../../templates/launchpad/deploy-web.yml.tera");
const GITIGNORE_TEMPLATE: &str = include_str!("../../../templates/launchpad/gitignore.tera");
const CLAUDE_MD_TEMPLATE: &str = include_str!("../../../templates/launchpad/claude.md.tera");
const README_TEMPLATE: &str = include_str!("../../../templates/launchpad/readme.md.tera");
const TODO_TEMPLATE: &str = include_str!("../../../templates/launchpad/todo.md.tera");
const SETTINGS_LOCAL_TEMPLATE: &str =
    include_str!("../../../templates/launchpad/settings.local.json.tera");

pub struct TemplateEngine {
    tera: Tera,
}

impl TemplateEngine {
    pub fn new() -> Result<Self> {
        let mut tera = Tera::default();

        tera.add_raw_template("wrangler.jsonc", WRANGLER_TEMPLATE)?;
        tera.add_raw_template("vite.config.ts", VITE_CONFIG_TEMPLATE)?;
        tera.add_raw_template("drizzle.config.ts", DRIZZLE_CONFIG_TEMPLATE)?;
        tera.add_raw_template("hono-entry.ts", HONO_ENTRY_TEMPLATE)?;
        tera.add_raw_template("axios-client.ts", AXIOS_CLIENT_TEMPLATE)?;
        tera.add_raw_template("config-worker.ts", CONFIG_WORKER_TEMPLATE)?;
        tera.add_raw_template("config-web.ts", CONFIG_WEB_TEMPLATE)?;
        tera.add_raw_template("schema.ts", SCHEMA_TEMPLATE)?;
        tera.add_raw_template("env.example", ENV_EXAMPLE_TEMPLATE)?;
        tera.add_raw_template("dev.vars.example", DEV_VARS_EXAMPLE_TEMPLATE)?;
        tera.add_raw_template("deploy-worker.yml", DEPLOY_WORKER_TEMPLATE)?;
        tera.add_raw_template("deploy-web.yml", DEPLOY_WEB_TEMPLATE)?;
        tera.add_raw_template("gitignore", GITIGNORE_TEMPLATE)?;
        tera.add_raw_template("claude.md", CLAUDE_MD_TEMPLATE)?;
        tera.add_raw_template("readme.md", README_TEMPLATE)?;
        tera.add_raw_template("todo.md", TODO_TEMPLATE)?;
        tera.add_raw_template("settings.local.json", SETTINGS_LOCAL_TEMPLATE)?;

        Ok(Self { tera })
    }

    pub fn render(&self, template_name: &str, context: &Context) -> Result<String> {
        self.tera
            .render(template_name, context)
            .map_err(|e| anyhow::anyhow!("Template rendering failed for '{}': {}", template_name, e))
    }

    pub fn wrangler_context(
        &self,
        config: &LaunchpadConfig,
        project: &ProjectConfig,
        port: u16,
    ) -> Context {
        let mut ctx = Context::new();
        ctx.insert("prefix", &config.name);
        ctx.insert("project_name", &project.name);
        ctx.insert("project_type", &serde_json::to_value(&project.project_type).unwrap());
        ctx.insert("today", &chrono::Local::now().format("%Y-%m-%d").to_string());
        ctx.insert("port", &port);
        ctx.insert("domain", &config.domain);
        ctx.insert("zone", &config.zone());
        ctx.insert("remote", &config.remote);
        ctx.insert("has_d1", &project.has_resource(&Resource::D1));
        ctx.insert("has_r2", &project.has_resource(&Resource::R2));
        ctx.insert("has_kv", &project.has_resource(&Resource::Kv));
        ctx.insert("has_queues", &project.has_resource(&Resource::Queues));
        ctx.insert("has_ai_gateway", &project.has_resource(&Resource::AiGateway));
        ctx.insert("d1_id", &"");
        ctx.insert("kv_id", &"");
        ctx
    }

    pub fn vite_context(&self, port: u16, api_port: Option<u16>) -> Context {
        let mut ctx = Context::new();
        ctx.insert("port", &port);
        ctx.insert("api_port", &api_port);
        ctx
    }

    pub fn worker_config_context(&self, project: &ProjectConfig) -> Context {
        let mut ctx = Context::new();
        ctx.insert(
            "auth",
            &project.auth.as_ref().map(|a| serde_json::to_value(a).unwrap()),
        );
        ctx.insert(
            "email",
            &project.email.as_ref().map(|e| serde_json::to_value(e).unwrap()),
        );
        ctx
    }

    pub fn web_config_context(&self, project: &ProjectConfig) -> Context {
        let mut ctx = Context::new();
        ctx.insert(
            "auth",
            &project.auth.as_ref().map(|a| serde_json::to_value(a).unwrap()),
        );
        ctx
    }

    pub fn env_example_context(&self, project: &ProjectConfig) -> Context {
        let mut ctx = Context::new();
        ctx.insert(
            "auth",
            &project.auth.as_ref().map(|a| serde_json::to_value(a).unwrap()),
        );
        ctx.insert(
            "email",
            &project.email.as_ref().map(|e| serde_json::to_value(e).unwrap()),
        );
        ctx
    }

    pub fn deploy_context(&self, project_name: &str, project_dir: &str) -> Context {
        let mut ctx = Context::new();
        ctx.insert("project_name", project_name);
        ctx.insert("project_dir", project_dir);
        ctx
    }

    pub fn project_files_context(
        &self,
        config: &LaunchpadConfig,
        ports: &[(String, u16)],
    ) -> Context {
        let mut ctx = Context::new();
        ctx.insert("name", &config.name);
        ctx.insert("prefix", &config.name);
        ctx.insert("description", &config.description);
        ctx.insert("domain", &config.domain);
        ctx.insert("has_api_worker", &config.has_api_worker());
        ctx.insert(
            "has_ios",
            &config.projects.iter().any(|p| p.project_type == ProjectType::Ios),
        );

        let projects: Vec<serde_json::Value> = config
            .projects
            .iter()
            .map(|p| {
                let port = ports
                    .iter()
                    .find(|(name, _)| name == &p.name)
                    .map(|(_, port)| *port);
                serde_json::json!({
                    "name": p.name,
                    "dir": p.name,
                    "description": format!("{} ({})", p.project_type.label(), if let Some(port) = port { format!("port {}", port) } else { "no port".to_string() }),
                    "env_example": matches!(p.project_type, ProjectType::Web),
                    "env_file": if p.project_type == ProjectType::Web { ".env.example" } else { ".dev.vars.example" },
                    "env_target": if p.project_type == ProjectType::Web { ".env" } else { ".dev.vars" },
                })
            })
            .collect();
        ctx.insert("projects", &projects);

        let worker_projects: Vec<serde_json::Value> = config
            .projects
            .iter()
            .filter(|p| p.is_worker())
            .map(|p| serde_json::json!({ "name": p.name }))
            .collect();
        ctx.insert("worker_projects", &worker_projects);

        let d1_projects: Vec<serde_json::Value> = config
            .projects
            .iter()
            .filter(|p| p.has_resource(&Resource::D1))
            .map(|p| serde_json::json!({ "dir": p.name }))
            .collect();
        ctx.insert("d1_projects", &d1_projects);

        let mut env_vars: Vec<serde_json::Value> = Vec::new();
        for p in &config.projects {
            match p.project_type {
                ProjectType::Web => {
                    env_vars.push(serde_json::json!({ "name": "VITE_VERSION", "project": p.name, "location": ".env" }));
                    if matches!(p.auth, Some(AuthProvider::Clerk)) {
                        env_vars.push(serde_json::json!({ "name": "VITE_CLERK_PUBLISHABLE_KEY", "project": p.name, "location": ".env" }));
                    }
                }
                ProjectType::ApiWorker | ProjectType::LightweightWorker => {
                    if matches!(p.auth, Some(AuthProvider::Clerk)) {
                        env_vars.push(serde_json::json!({ "name": "CLERK_SECRET_KEY", "project": p.name, "location": ".dev.vars" }));
                    }
                    if matches!(p.email, Some(EmailProvider::Resend)) {
                        env_vars.push(serde_json::json!({ "name": "RESEND_API_KEY", "project": p.name, "location": ".dev.vars" }));
                    }
                }
                _ => {}
            }
        }
        ctx.insert("env_vars", &env_vars);
        ctx.insert("has_env_vars", &!env_vars.is_empty());

        ctx
    }
}

pub fn write_template(content: &str, path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, content)?;
    Ok(())
}
