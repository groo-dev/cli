use super::config::{AuthProvider, LaunchpadConfig, ProjectType};
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
        step_shadcn_init(project, root, state, ui).await?;
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

async fn step_shadcn_init(
    project: &super::config::ProjectConfig,
    root: &Path,
    state: &mut LaunchpadState,
    ui: &Ui,
) -> Result<()> {
    if !project.has_feature_type("shadcn") {
        return Ok(());
    }

    let step = "shadcn_init";
    if state.is_step_complete(step, Some(&project.name)) {
        ui.skipped(&format!("shadcn init for {}", project.name));
        return Ok(());
    }

    let project_dir = root.join(&project.name);

    // shadcn init requires tailwind CSS import and path aliases set up first
    // 1. Add tailwind import to index.css
    let css_path = project_dir.join("src/index.css");
    std::fs::write(&css_path, "@import \"tailwindcss\";\n")?;

    // 2. Add path alias to tsconfig.json and tsconfig.app.json
    for tsconfig_name in ["tsconfig.json", "tsconfig.app.json"] {
        let tsconfig_path = project_dir.join(tsconfig_name);
        if tsconfig_path.exists() {
            let content = std::fs::read_to_string(&tsconfig_path)?;
            if let Ok(mut tsconfig) = serde_json::from_str::<serde_json::Value>(&content) {
                // Ensure compilerOptions exists (tsconfig.json may be references-only)
                if tsconfig.get("compilerOptions").is_none() {
                    tsconfig
                        .as_object_mut()
                        .unwrap()
                        .insert("compilerOptions".to_string(), serde_json::json!({}));
                }
                if let Some(compiler_options) = tsconfig
                    .get_mut("compilerOptions")
                    .and_then(|c| c.as_object_mut())
                {
                    compiler_options
                        .insert("baseUrl".to_string(), serde_json::json!("."));
                    compiler_options.insert(
                        "paths".to_string(),
                        serde_json::json!({ "@/*": ["./src/*"] }),
                    );
                }
                let updated = serde_json::to_string_pretty(&tsconfig)?;
                std::fs::write(&tsconfig_path, format!("{}\n", updated))?;
            }
        }
    }

    ui.run_command(
        &format!("Initialized shadcn for {}", project.name),
        "npx shadcn@latest init -d -y",
        &project_dir,
    )
    .await?;

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
            ProjectType::Worker => {
                let ctx = engine.wrangler_context(config, project, port);
                let content = engine.render("wrangler.jsonc", &ctx)?;
                templates::write_template(&content, &project_dir.join("wrangler.jsonc"))?;
                ui.success(&format!("{}/wrangler.jsonc", project.name));

                if project.has_feature_type("drizzle") {
                    let ctx = tera::Context::new();
                    let content = engine.render("drizzle.config.ts", &ctx)?;
                    templates::write_template(&content, &project_dir.join("drizzle.config.ts"))?;
                    ui.success(&format!("{}/drizzle.config.ts", project.name));
                }
            }
            ProjectType::Web => {
                let api_port = config.hono_worker_port(port_map);
                let ctx = engine.vite_context(project, port, api_port);
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
                ProjectType::Worker => {
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

                    if project.has_feature_type("drizzle") {
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
            ProjectType::Worker => {
                let src_dir = project_dir.join("src");
                std::fs::create_dir_all(&src_dir)?;

                // Hono entry point (if hono feature)
                if project.has_feature_type("hono") {
                    let mut ctx = tera::Context::new();
                    ctx.insert(
                        "has_auth_clerk",
                        &matches!(
                            project.auth_provider(),
                            Some(AuthProvider::Clerk)
                        ),
                    );
                    ctx.insert(
                        "has_auth_better_auth",
                        &matches!(
                            project.auth_provider(),
                            Some(AuthProvider::BetterAuth)
                        ),
                    );
                    let content = engine.render("hono-entry.ts", &ctx)?;
                    templates::write_template(&content, &src_dir.join("index.ts"))?;
                    ui.success(&format!("{}/src/index.ts (Hono entry)", project.name));
                }

                // config.ts
                let ctx = engine.worker_config_context(project);
                let content = engine.render("config-worker.ts", &ctx)?;
                templates::write_template(&content, &src_dir.join("config.ts"))?;
                ui.success(&format!("{}/src/config.ts", project.name));

                // schema.ts (if drizzle feature)
                if project.has_feature_type("drizzle") {
                    let ctx = tera::Context::new();
                    let content = engine.render("schema.ts", &ctx)?;
                    let db_dir = src_dir.join("db");
                    std::fs::create_dir_all(&db_dir)?;
                    templates::write_template(&content, &db_dir.join("schema.ts"))?;
                    ui.success(&format!("{}/src/db/schema.ts", project.name));
                }

                // auth schema (if auth(better-auth) + drizzle)
                if matches!(project.auth_provider(), Some(AuthProvider::BetterAuth))
                    && project.has_feature_type("drizzle")
                {
                    let ctx = tera::Context::new();
                    let content = engine.render("auth-schema.ts", &ctx)?;
                    let schema_dir = src_dir.join("db").join("schema");
                    std::fs::create_dir_all(&schema_dir)?;
                    templates::write_template(&content, &schema_dir.join("auth.ts"))?;
                    ui.success(&format!(
                        "{}/src/db/schema/auth.ts (Better Auth schema)",
                        project.name
                    ));
                }

                // auth middleware (if hono + auth)
                if project.has_feature_type("hono") {
                    if matches!(project.auth_provider(), Some(AuthProvider::Clerk)) {
                        let ctx = tera::Context::new();
                        let content = engine.render("auth-middleware-clerk.ts", &ctx)?;
                        let middleware_dir = src_dir.join("middleware");
                        std::fs::create_dir_all(&middleware_dir)?;
                        templates::write_template(
                            &content,
                            &middleware_dir.join("auth.ts"),
                        )?;
                        ui.success(&format!(
                            "{}/src/middleware/auth.ts (Clerk)",
                            project.name
                        ));
                    } else if matches!(
                        project.auth_provider(),
                        Some(AuthProvider::BetterAuth)
                    ) {
                        let ctx = tera::Context::new();
                        let content =
                            engine.render("auth-middleware-better-auth.ts", &ctx)?;
                        let middleware_dir = src_dir.join("middleware");
                        std::fs::create_dir_all(&middleware_dir)?;
                        templates::write_template(
                            &content,
                            &middleware_dir.join("auth.ts"),
                        )?;
                        ui.success(&format!(
                            "{}/src/middleware/auth.ts (Better Auth)",
                            project.name
                        ));
                    }
                }
            }
            ProjectType::Web => {
                let src_dir = project_dir.join("src");

                // main.tsx (overwrite Vite-scaffolded one)
                let ctx = engine.main_web_context(project);
                let content = engine.render("main-web.tsx", &ctx)?;
                templates::write_template(&content, &src_dir.join("main.tsx"))?;
                ui.success(&format!("{}/src/main.tsx", project.name));

                // TanStack Router routes
                if project.has_feature_type("tanstack-router") {
                    let routes_dir = src_dir.join("routes");
                    std::fs::create_dir_all(&routes_dir)?;

                    let ctx = tera::Context::new();
                    let content = engine.render("root-route.tsx", &ctx)?;
                    templates::write_template(&content, &routes_dir.join("__root.tsx"))?;
                    ui.success(&format!("{}/src/routes/__root.tsx", project.name));

                    let content = engine.render("index-route.tsx", &ctx)?;
                    templates::write_template(&content, &routes_dir.join("index.tsx"))?;
                    ui.success(&format!("{}/src/routes/index.tsx", project.name));

                    // Delete Vite-scaffolded files replaced by router
                    let app_tsx = src_dir.join("App.tsx");
                    if app_tsx.exists() {
                        std::fs::remove_file(&app_tsx)?;
                    }
                    let app_css = src_dir.join("App.css");
                    if app_css.exists() {
                        std::fs::remove_file(&app_css)?;
                    }
                }

                // axios client (if axios feature)
                if project.has_feature_type("axios") {
                    let ctx = tera::Context::new();
                    let content = engine.render("axios-client.ts", &ctx)?;
                    let lib_dir = src_dir.join("lib");
                    std::fs::create_dir_all(&lib_dir)?;
                    templates::write_template(&content, &lib_dir.join("api.ts"))?;
                    ui.success(&format!("{}/src/lib/api.ts (Axios client)", project.name));
                }

                // config.ts
                let ctx = engine.web_config_context(project);
                let content = engine.render("config-web.ts", &ctx)?;
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
            ProjectType::Worker => {
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
                        "\"database_id\": \"xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx\"",
                        &format!("\"database_id\": \"{}\"", resource.id),
                    );
                }
                "kv" => {
                    content = content.replace(
                        "\"id\": \"xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx\"",
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

    // Run migrations for workers with drizzle feature
    let drizzle_workers: Vec<_> = config
        .projects
        .iter()
        .filter(|p| p.has_feature_type("drizzle"))
        .collect();

    if drizzle_workers.is_empty() {
        state.mark_complete(step, None);
        state.save()?;
        return Ok(());
    }

    ui.newline();
    ui.section("Database setup:");

    for project in drizzle_workers {
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
            ProjectType::Worker => {
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
