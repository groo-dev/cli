use super::config::{LaunchpadConfig, ProjectType, Resource};
use super::deps;
use super::ports;
use super::resources;
use super::scaffold;
use super::state::LaunchpadState;
use super::templates::{self, TemplateEngine};
use super::ui::Ui;
use anyhow::Result;
use std::path::Path;

pub async fn run_pipeline(
    config: &LaunchpadConfig,
    _config_content: &str,
    root: &Path,
    state: &mut LaunchpadState,
    ui: &Ui,
) -> Result<()> {
    let engine = TemplateEngine::new()?;

    // Step 1: Check if resuming
    if !state.completed_steps.is_empty() {
        ui.section("Resuming from previous run...");
        ui.newline();
    }

    // Step 2: Create root directory
    step_create_root(root, state, ui)?;

    // Step 3: Scaffold and install deps per project
    for project in &config.projects {
        step_scaffold_project(config, project, root, state, ui).await?;
        step_install_deps(config, project, root, state, ui).await?;
    }

    // Step 4: Generate ports
    let port_map = step_generate_ports(config, state, ui)?;

    // Step 5: Write config files
    step_write_config_files(config, &engine, &port_map, root, state, ui)?;

    // Step 6: Write package.json scripts
    step_write_package_scripts(config, &port_map, root, state, ui)?;

    // Step 7: Write boilerplate code
    step_write_boilerplate(config, &engine, root, state, ui)?;

    // Step 8: Write env example files
    step_write_env_examples(config, &engine, root, state, ui)?;

    // Step 9: Create Cloudflare resources
    step_create_resources(config, root, state, ui).await?;

    // Step 10: Bind resource IDs to wrangler.jsonc
    step_bind_resource_ids(config, state, root, ui)?;

    // Step 11: Run cf-typegen per worker
    step_cf_typegen(config, root, state, ui).await?;

    // Step 12: Run db:generate + db:migrate:local for D1 workers
    step_db_migrations(config, root, state, ui).await?;

    // Step 13: Write project files
    step_write_project_files(config, &engine, &port_map, root, state, ui)?;

    // Step 14: Git init + initial commit
    step_git_init(root, state, ui).await?;

    state.delete()?;
    ui.done();

    Ok(())
}

fn step_create_root(root: &Path, state: &mut LaunchpadState, ui: &Ui) -> Result<()> {
    let step = "create_root";
    if state.is_step_complete(step, None) {
        ui.skipped("Create root directory");
        return Ok(());
    }

    if !root.exists() {
        std::fs::create_dir_all(root)?;
        ui.success("Created root directory");
    }

    state.mark_complete(step, None);
    state.save()?;
    Ok(())
}

async fn step_scaffold_project(
    _config: &LaunchpadConfig,
    project: &super::config::ProjectConfig,
    root: &Path,
    state: &mut LaunchpadState,
    ui: &Ui,
) -> Result<()> {
    let step = "scaffold";
    if state.is_step_complete(step, Some(&project.name)) {
        ui.skipped(&format!("Scaffold {}", project.name));
        return Ok(());
    }

    ui.section(&format!("{}:", project.name));
    scaffold::scaffold_project(ui, &project.name, &project.project_type, root).await?;

    state.mark_complete(step, Some(&project.name));
    state.save()?;
    Ok(())
}

async fn step_install_deps(
    _config: &LaunchpadConfig,
    project: &super::config::ProjectConfig,
    root: &Path,
    state: &mut LaunchpadState,
    ui: &Ui,
) -> Result<()> {
    let step = "install_deps";
    if state.is_step_complete(step, Some(&project.name)) {
        ui.skipped(&format!("Install deps for {}", project.name));
        return Ok(());
    }

    if matches!(project.project_type, ProjectType::Ios | ProjectType::Android) {
        state.mark_complete(step, Some(&project.name));
        state.save()?;
        return Ok(());
    }

    let project_dir = root.join(&project.name);
    deps::install_deps(ui, project, &project_dir).await?;

    state.mark_complete(step, Some(&project.name));
    state.save()?;
    Ok(())
}

fn step_generate_ports(
    config: &LaunchpadConfig,
    state: &mut LaunchpadState,
    ui: &Ui,
) -> Result<Vec<(String, u16)>> {
    let step = "generate_ports";

    // Collect projects that need ports (non-iOS/Android)
    let port_projects: Vec<&str> = config
        .projects
        .iter()
        .filter(|p| !matches!(p.project_type, ProjectType::Ios | ProjectType::Android))
        .map(|p| p.name.as_str())
        .collect();

    if state.is_step_complete(step, None) {
        ui.skipped("Generate dev ports");
        // Regenerate ports deterministically for downstream steps
        let generated = ports::generate_ports(port_projects.len());
        return Ok(port_projects
            .into_iter()
            .zip(generated)
            .map(|(name, port)| (name.to_string(), port))
            .collect());
    }

    let generated = ports::generate_ports(port_projects.len());
    let port_map: Vec<(String, u16)> = port_projects
        .into_iter()
        .zip(generated)
        .map(|(name, port)| (name.to_string(), port))
        .collect();

    for (name, port) in &port_map {
        ui.success(&format!("{} → port {}", name, port));
    }

    state.mark_complete(step, None);
    state.save()?;
    Ok(port_map)
}

fn step_write_config_files(
    config: &LaunchpadConfig,
    engine: &TemplateEngine,
    port_map: &[(String, u16)],
    root: &Path,
    state: &mut LaunchpadState,
    ui: &Ui,
) -> Result<()> {
    let step = "write_config_files";
    if state.is_step_complete(step, None) {
        ui.skipped("Write config files");
        return Ok(());
    }

    ui.newline();
    ui.section("Config files:");

    for project in &config.projects {
        let project_dir = root.join(&project.name);
        let port = port_map
            .iter()
            .find(|(name, _)| name == &project.name)
            .map(|(_, p)| *p)
            .unwrap_or(8787);

        match project.project_type {
            ProjectType::ApiWorker | ProjectType::LightweightWorker => {
                let ctx = engine.wrangler_context(config, project, port);
                let content = engine.render("wrangler.jsonc", &ctx)?;
                templates::write_template(&content, &project_dir.join("wrangler.jsonc"))?;
                ui.success(&format!("{}/wrangler.jsonc", project.name));

                if project.has_resource(&Resource::D1) {
                    let ctx = tera::Context::new();
                    let content = engine.render("drizzle.config.ts", &ctx)?;
                    templates::write_template(&content, &project_dir.join("drizzle.config.ts"))?;
                    ui.success(&format!("{}/drizzle.config.ts", project.name));
                }
            }
            ProjectType::Web => {
                let api_port = config.api_worker_port(port_map);
                let ctx = engine.vite_context(port, api_port);
                let content = engine.render("vite.config.ts", &ctx)?;
                templates::write_template(&content, &project_dir.join("vite.config.ts"))?;
                ui.success(&format!("{}/vite.config.ts", project.name));
            }
            _ => {}
        }
    }

    state.mark_complete(step, None);
    state.save()?;
    Ok(())
}

fn step_write_package_scripts(
    config: &LaunchpadConfig,
    port_map: &[(String, u16)],
    root: &Path,
    state: &mut LaunchpadState,
    ui: &Ui,
) -> Result<()> {
    let step = "write_package_scripts";
    if state.is_step_complete(step, None) {
        ui.skipped("Write package.json scripts");
        return Ok(());
    }

    ui.newline();
    ui.section("Package scripts:");

    for project in &config.projects {
        let project_dir = root.join(&project.name);
        let pkg_path = project_dir.join("package.json");

        if !pkg_path.exists() {
            continue;
        }

        let pkg_content = std::fs::read_to_string(&pkg_path)?;
        let mut pkg: serde_json::Value = serde_json::from_str(&pkg_content)?;

        let port = port_map
            .iter()
            .find(|(name, _)| name == &project.name)
            .map(|(_, p)| *p)
            .unwrap_or(8787);

        if let Some(scripts) = pkg.get_mut("scripts").and_then(|s| s.as_object_mut()) {
            match project.project_type {
                ProjectType::ApiWorker | ProjectType::LightweightWorker => {
                    scripts.insert(
                        "dev".to_string(),
                        serde_json::json!(format!("wrangler dev --port {}", port)),
                    );
                    scripts.insert(
                        "deploy".to_string(),
                        serde_json::json!("wrangler deploy"),
                    );
                    scripts.insert(
                        "cf-typegen".to_string(),
                        serde_json::json!("wrangler types"),
                    );

                    if project.has_resource(&Resource::D1) {
                        scripts.insert(
                            "db:generate".to_string(),
                            serde_json::json!("drizzle-kit generate"),
                        );
                        scripts.insert(
                            "db:migrate:local".to_string(),
                            serde_json::json!(format!(
                                "wrangler d1 migrations apply {}-d1 --local",
                                config.name
                            )),
                        );
                        scripts.insert(
                            "db:migrate:remote".to_string(),
                            serde_json::json!(format!(
                                "wrangler d1 migrations apply {}-d1 --remote",
                                config.name
                            )),
                        );
                    }
                }
                ProjectType::Web => {
                    scripts.insert(
                        "dev".to_string(),
                        serde_json::json!(format!("vite --port {}", port)),
                    );
                    scripts.insert(
                        "deploy".to_string(),
                        serde_json::json!("wrangler pages deploy dist"),
                    );
                }
                _ => {}
            }
        }

        let updated = serde_json::to_string_pretty(&pkg)?;
        std::fs::write(&pkg_path, format!("{}\n", updated))?;
        ui.success(&format!("{}/package.json scripts", project.name));
    }

    state.mark_complete(step, None);
    state.save()?;
    Ok(())
}

fn step_write_boilerplate(
    config: &LaunchpadConfig,
    engine: &TemplateEngine,
    root: &Path,
    state: &mut LaunchpadState,
    ui: &Ui,
) -> Result<()> {
    let step = "write_boilerplate";
    if state.is_step_complete(step, None) {
        ui.skipped("Write boilerplate code");
        return Ok(());
    }

    ui.newline();
    ui.section("Boilerplate:");

    for project in &config.projects {
        let project_dir = root.join(&project.name);

        match project.project_type {
            ProjectType::ApiWorker => {
                // hono entry point
                let ctx = tera::Context::new();
                let content = engine.render("hono-entry.ts", &ctx)?;
                let src_dir = project_dir.join("src");
                std::fs::create_dir_all(&src_dir)?;
                templates::write_template(&content, &src_dir.join("index.ts"))?;
                ui.success(&format!("{}/src/index.ts (Hono entry)", project.name));

                // config.ts
                let ctx = engine.worker_config_context(project);
                let content = engine.render("config-worker.ts", &ctx)?;
                templates::write_template(&content, &src_dir.join("config.ts"))?;
                ui.success(&format!("{}/src/config.ts", project.name));

                // schema.ts for D1
                if project.has_resource(&Resource::D1) {
                    let ctx = tera::Context::new();
                    let content = engine.render("schema.ts", &ctx)?;
                    let db_dir = src_dir.join("db");
                    std::fs::create_dir_all(&db_dir)?;
                    templates::write_template(&content, &db_dir.join("schema.ts"))?;
                    ui.success(&format!("{}/src/db/schema.ts", project.name));
                }
            }
            ProjectType::LightweightWorker => {
                // config.ts
                let ctx = engine.worker_config_context(project);
                let content = engine.render("config-worker.ts", &ctx)?;
                let src_dir = project_dir.join("src");
                std::fs::create_dir_all(&src_dir)?;
                templates::write_template(&content, &src_dir.join("config.ts"))?;
                ui.success(&format!("{}/src/config.ts", project.name));
            }
            ProjectType::Web => {
                // axios client
                let ctx = tera::Context::new();
                let content = engine.render("axios-client.ts", &ctx)?;
                let lib_dir = project_dir.join("src").join("lib");
                std::fs::create_dir_all(&lib_dir)?;
                templates::write_template(&content, &lib_dir.join("api.ts"))?;
                ui.success(&format!("{}/src/lib/api.ts (Axios client)", project.name));

                // config.ts
                let ctx = engine.web_config_context(project);
                let content = engine.render("config-web.ts", &ctx)?;
                let src_dir = project_dir.join("src");
                templates::write_template(&content, &src_dir.join("config.ts"))?;
                ui.success(&format!("{}/src/config.ts", project.name));
            }
            _ => {}
        }
    }

    state.mark_complete(step, None);
    state.save()?;
    Ok(())
}

fn step_write_env_examples(
    config: &LaunchpadConfig,
    engine: &TemplateEngine,
    root: &Path,
    state: &mut LaunchpadState,
    ui: &Ui,
) -> Result<()> {
    let step = "write_env_examples";
    if state.is_step_complete(step, None) {
        ui.skipped("Write env example files");
        return Ok(());
    }

    ui.newline();
    ui.section("Environment files:");

    for project in &config.projects {
        let project_dir = root.join(&project.name);

        match project.project_type {
            ProjectType::Web => {
                let ctx = engine.env_example_context(project);
                let content = engine.render("env.example", &ctx)?;
                templates::write_template(&content, &project_dir.join(".env.example"))?;
                ui.success(&format!("{}/.env.example", project.name));
            }
            ProjectType::ApiWorker | ProjectType::LightweightWorker => {
                let ctx = engine.env_example_context(project);
                let content = engine.render("dev.vars.example", &ctx)?;
                templates::write_template(&content, &project_dir.join(".dev.vars.example"))?;
                ui.success(&format!("{}/.dev.vars.example", project.name));
            }
            _ => {}
        }
    }

    state.mark_complete(step, None);
    state.save()?;
    Ok(())
}

async fn step_create_resources(
    config: &LaunchpadConfig,
    root: &Path,
    state: &mut LaunchpadState,
    ui: &Ui,
) -> Result<()> {
    let step = "create_resources";
    if state.is_step_complete(step, None) {
        ui.skipped("Create Cloudflare resources");
        return Ok(());
    }

    if !config.create_resources {
        ui.success("Skipped Cloudflare resource creation (create_resources: false)");
        state.mark_complete(step, None);
        state.save()?;
        return Ok(());
    }

    ui.newline();
    ui.section("Cloudflare resources:");

    resources::create_resources(ui, config, state, root).await?;

    state.mark_complete(step, None);
    state.save()?;
    Ok(())
}

fn step_bind_resource_ids(
    config: &LaunchpadConfig,
    state: &LaunchpadState,
    root: &Path,
    ui: &Ui,
) -> Result<()> {
    let step = "bind_resource_ids";
    // Need mutable borrow for state but we only read it here
    // Check completion manually
    if state.is_step_complete(step, None) {
        ui.skipped("Bind resource IDs");
        return Ok(());
    }

    if state.created_resources.is_empty() {
        return Ok(());
    }

    ui.newline();
    ui.section("Binding resource IDs:");

    for project in &config.projects {
        if !project.is_worker() {
            continue;
        }

        let wrangler_path = root.join(&project.name).join("wrangler.jsonc");
        if !wrangler_path.exists() {
            continue;
        }

        let mut content = std::fs::read_to_string(&wrangler_path)?;

        for resource in &state.created_resources {
            if resource.id.is_empty() {
                continue;
            }

            match resource.resource_type.as_str() {
                "d1" => {
                    content = content.replace(
                        "\"database_id\": \"\"",
                        &format!("\"database_id\": \"{}\"", resource.id),
                    );
                }
                "kv" => {
                    content = content.replace(
                        "\"id\": \"\"",
                        &format!("\"id\": \"{}\"", resource.id),
                    );
                }
                _ => {}
            }
        }

        std::fs::write(&wrangler_path, &content)?;
        ui.success(&format!("{}/wrangler.jsonc updated", project.name));
    }

    Ok(())
}

async fn step_cf_typegen(
    config: &LaunchpadConfig,
    root: &Path,
    state: &mut LaunchpadState,
    ui: &Ui,
) -> Result<()> {
    let step = "cf_typegen";
    if state.is_step_complete(step, None) {
        ui.skipped("Generate Cloudflare types");
        return Ok(());
    }

    let workers: Vec<_> = config
        .projects
        .iter()
        .filter(|p| p.is_worker())
        .collect();

    if workers.is_empty() {
        state.mark_complete(step, None);
        state.save()?;
        return Ok(());
    }

    ui.newline();
    ui.section("Type generation:");

    for project in workers {
        let project_dir = root.join(&project.name);
        ui.run_command(
            &format!("Generated types for {}", project.name),
            "npm run cf-typegen",
            &project_dir,
        )
        .await?;
    }

    state.mark_complete(step, None);
    state.save()?;
    Ok(())
}

async fn step_db_migrations(
    config: &LaunchpadConfig,
    root: &Path,
    state: &mut LaunchpadState,
    ui: &Ui,
) -> Result<()> {
    let step = "db_migrations";
    if state.is_step_complete(step, None) {
        ui.skipped("Database migrations");
        return Ok(());
    }

    let d1_projects: Vec<_> = config
        .projects
        .iter()
        .filter(|p| p.has_resource(&Resource::D1))
        .collect();

    if d1_projects.is_empty() {
        state.mark_complete(step, None);
        state.save()?;
        return Ok(());
    }

    ui.newline();
    ui.section("Database setup:");

    for project in d1_projects {
        let project_dir = root.join(&project.name);

        ui.run_command(
            &format!("Generated migrations for {}", project.name),
            "npm run db:generate",
            &project_dir,
        )
        .await?;

        ui.run_command(
            &format!("Applied local migrations for {}", project.name),
            "npm run db:migrate:local",
            &project_dir,
        )
        .await?;
    }

    state.mark_complete(step, None);
    state.save()?;
    Ok(())
}

fn step_write_project_files(
    config: &LaunchpadConfig,
    engine: &TemplateEngine,
    port_map: &[(String, u16)],
    root: &Path,
    state: &mut LaunchpadState,
    ui: &Ui,
) -> Result<()> {
    let step = "write_project_files";
    if state.is_step_complete(step, None) {
        ui.skipped("Write project files");
        return Ok(());
    }

    ui.newline();
    ui.section("Project files:");

    let ctx = engine.project_files_context(config, port_map);

    // CLAUDE.md
    let content = engine.render("claude.md", &ctx)?;
    templates::write_template(&content, &root.join("CLAUDE.md"))?;
    ui.success("CLAUDE.md");

    // README.md
    let content = engine.render("readme.md", &ctx)?;
    templates::write_template(&content, &root.join("README.md"))?;
    ui.success("README.md");

    // TODO.md
    let content = engine.render("todo.md", &ctx)?;
    templates::write_template(&content, &root.join("TODO.md"))?;
    ui.success("TODO.md");

    // .gitignore
    let content = engine.render("gitignore", &ctx)?;
    templates::write_template(&content, &root.join(".gitignore"))?;
    ui.success(".gitignore");

    // GitHub Actions workflows
    for project in &config.projects {
        match project.project_type {
            ProjectType::ApiWorker | ProjectType::LightweightWorker => {
                let deploy_ctx = engine.deploy_context(&project.name, &project.name);
                let content = engine.render("deploy-worker.yml", &deploy_ctx)?;
                let workflow_path = root
                    .join(".github")
                    .join("workflows")
                    .join(format!("deploy-{}.yml", project.name));
                templates::write_template(&content, &workflow_path)?;
                ui.success(&format!(".github/workflows/deploy-{}.yml", project.name));
            }
            ProjectType::Web => {
                let deploy_ctx = engine.deploy_context(&project.name, &project.name);
                let content = engine.render("deploy-web.yml", &deploy_ctx)?;
                let workflow_path = root
                    .join(".github")
                    .join("workflows")
                    .join(format!("deploy-{}.yml", project.name));
                templates::write_template(&content, &workflow_path)?;
                ui.success(&format!(".github/workflows/deploy-{}.yml", project.name));
            }
            _ => {}
        }
    }

    // settings.local.json
    let content = engine.render("settings.local.json", &ctx)?;
    let settings_path = root.join(".claude").join("settings.local.json");
    templates::write_template(&content, &settings_path)?;
    ui.success(".claude/settings.local.json");

    state.mark_complete(step, None);
    state.save()?;
    Ok(())
}

async fn step_git_init(root: &Path, state: &mut LaunchpadState, ui: &Ui) -> Result<()> {
    let step = "git_init";
    if state.is_step_complete(step, None) {
        ui.skipped("Git init");
        return Ok(());
    }

    ui.newline();
    ui.section("Git:");

    ui.run_command("Initialized git repository", "git init", root)
        .await?;

    ui.run_command("Staged all files", "git add -A", root)
        .await?;

    ui.run_command(
        "Created initial commit",
        "git commit -m \"Initial commit from groo launchpad\"",
        root,
    )
    .await?;

    state.mark_complete(step, None);
    state.save()?;
    Ok(())
}
