# Launchpad CLI Command Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a `groo launchpad` command that takes a JSON config and deterministically scaffolds a complete multi-project repo — scaffolding, dependencies, config files, Cloudflare resources, project docs, and git init.

**Architecture:** The LLM collects requirements and writes a config JSON. The CLI validates it, then executes a linear pipeline of steps: scaffold projects via npm, install deps, generate config files from Tera templates, create Cloudflare resources, write project docs, and git init. State is tracked in `.launchpad-state.json` for resume-on-failure. A terminal UI shows live progress with collapsing log output.

**Tech Stack:** Rust, Tera (template engine), crossterm + console (terminal UI), serde (config parsing), tokio (async process spawning)

**Spec:** `docs/superpowers/specs/2026-04-06-launchpad-cli-design.md`

---

## File Structure

### New files

- `src/commands/launchpad/mod.rs` — Command entry point. Parses args, loads config, runs validation, orchestrates the pipeline, handles `--clean` flag.
- `src/commands/launchpad/config.rs` — `LaunchpadConfig` struct with serde deserialization. Enums: `ProjectType`, `AuthProvider`, `EmailProvider`, `Resource`. Derives `Serialize`/`Deserialize`.
- `src/commands/launchpad/validation.rs` — `validate(config, root_path) -> Result<(), Vec<String>>`. Each business rule is a separate function returning optional error string. Collects all errors before returning.
- `src/commands/launchpad/state.rs` — `LaunchpadState` struct. Read/write `.launchpad-state.json`. Track completed steps, created resources. Methods: `load()`, `save()`, `is_step_complete()`, `mark_complete()`, `mark_failed()`, `config_hash()`.
- `src/commands/launchpad/ui.rs` — Terminal UI with spinner, streaming log output, collapse-on-complete. `StepRunner` struct wraps async command execution with live output display.
- `src/commands/launchpad/pipeline.rs` — The 14-step execution pipeline. Each step is a function. Uses `StepRunner` for display. Checks state for resume. Records progress.
- `src/commands/launchpad/scaffold.rs` — `scaffold_project()` function. Runs `npm create vite` or `npm create cloudflare` with non-interactive flags. Returns the created directory path.
- `src/commands/launchpad/deps.rs` — `install_deps()` function. Builds the npm install command string per project type + auth + email selection.
- `src/commands/launchpad/templates.rs` — Tera template loading (embedded via `include_str!`), context building from config, and rendering. One public function per template category.
- `src/commands/launchpad/ports.rs` — `generate_ports(count) -> Vec<u16>`. Generates unique random 5-digit ports (10000-65535), checks none are in use.
- `src/commands/launchpad/resources.rs` — `create_resource()` function. Runs wrangler CLI commands to create D1/R2/KV/Queues/Pages. Parses output for resource IDs.
- `src/commands/launchpad/clean.rs` — `clean_previous_run()` function. Reads state file, deletes directories, deletes Cloudflare resources, removes state file.
- `templates/launchpad/wrangler.jsonc.tera` — Wrangler config template with conditional resource bindings, routes, and remote flag.
- `templates/launchpad/vite.config.ts.tera` — Vite config template with conditional API proxy.
- `templates/launchpad/drizzle.config.ts.tera` — Drizzle config template.
- `templates/launchpad/hono-entry.ts.tera` — API worker entry point template.
- `templates/launchpad/axios-client.ts.tera` — Web project axios API client template.
- `templates/launchpad/config-worker.ts.tera` — Worker config.ts template with auth/email secret getters.
- `templates/launchpad/config-web.ts.tera` — Web config.ts template with VITE_ env var getters.
- `templates/launchpad/schema.ts.tera` — Drizzle starter schema template.
- `templates/launchpad/env.example.tera` — Web .env.example template.
- `templates/launchpad/dev.vars.example.tera` — Worker .dev.vars.example template.
- `templates/launchpad/deploy-worker.yml.tera` — GitHub Actions worker deploy workflow template.
- `templates/launchpad/deploy-web.yml.tera` — GitHub Actions web deploy workflow template.
- `templates/launchpad/gitignore.tera` — Root .gitignore template.
- `templates/launchpad/claude.md.tera` — CLAUDE.md template with coding practices.
- `templates/launchpad/readme.md.tera` — README.md template.
- `templates/launchpad/todo.md.tera` — TODO.md template.
- `templates/launchpad/settings.local.json.tera` — .claude/settings.local.json template.

### Modified files

- `Cargo.toml` — Add `tera` dependency, add `sha2` feature if needed for config hashing.
- `src/commands/mod.rs` — Add `pub mod launchpad;`.
- `src/main.rs` — Add `Launchpad` variant to `Commands` enum with `--config` and `--clean` args, add match arm.

---

## Chunk 1: Foundation — Config, Validation, State

### Task 1: Add tera dependency and wire up the launchpad command

**Files:**
- Modify: `Cargo.toml`
- Modify: `src/commands/mod.rs`
- Modify: `src/main.rs`
- Create: `src/commands/launchpad/mod.rs`

- [ ] **Step 1: Add tera to Cargo.toml**

Add under the `[dependencies]` section:

```toml
# Launchpad template engine
tera = "1.20"
```

- [ ] **Step 2: Add launchpad module to commands/mod.rs**

Add to `src/commands/mod.rs`:

```rust
pub mod launchpad;
```

- [ ] **Step 3: Add Launchpad command variant to main.rs**

In `src/main.rs`, add to the `Commands` enum:

```rust
    /// Scaffold a new project from config
    Launchpad {
        /// Path to the launchpad config JSON file
        #[arg(short, long)]
        config: PathBuf,
        /// Clean previous failed run before starting fresh
        #[arg(long)]
        clean: bool,
    },
```

Add the match arm in `main()`:

```rust
        Commands::Launchpad { config, clean } => {
            commands::launchpad::run(config, clean).await
        }
```

Add `use std::path::PathBuf;` if not already imported (it is already imported).

- [ ] **Step 4: Create the launchpad module entry point**

Create `src/commands/launchpad/mod.rs`:

```rust
mod config;
mod validation;

use anyhow::{bail, Result};
use std::path::PathBuf;

pub async fn run(config_path: PathBuf, clean: bool) -> Result<()> {
    // Load and parse config
    let config_content = std::fs::read_to_string(&config_path)
        .map_err(|e| anyhow::anyhow!("Failed to read config file '{}': {}", config_path.display(), e))?;

    let config: config::LaunchpadConfig = serde_json::from_str(&config_content)
        .map_err(|e| anyhow::anyhow!("Invalid config JSON: {}", e))?;

    // Determine root directory
    let root = if config.root == "." {
        std::env::current_dir()?
    } else {
        std::env::current_dir()?.join(&config.root)
    };

    // Validate
    let errors = validation::validate(&config, &root);
    if !errors.is_empty() {
        let count = errors.len();
        let mut msg = format!("\n  Launchpad 🚀\n\n  ✗ Config validation failed ({} error{}):\n", count, if count == 1 { "" } else { "s" });
        for (i, error) in errors.iter().enumerate() {
            msg.push_str(&format!("\n  {}. {}\n", i + 1, error));
        }
        bail!("{}", msg);
    }

    println!("\n  Launchpad 🚀\n");
    println!("  Config validated. Pipeline not yet implemented.\n");

    Ok(())
}
```

- [ ] **Step 5: Verify it compiles**

Run: `cargo build 2>&1 | tail -5`

Expected: Build succeeds (config and validation modules created in next tasks, so add placeholder files first).

Create placeholder `src/commands/launchpad/config.rs`:

```rust
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct LaunchpadConfig {
    pub name: String,
    pub root: String,
    pub description: String,
    pub domain: Option<String>,
    pub projects: Vec<ProjectConfig>,
    pub create_resources: bool,
    pub remote: bool,
}

#[derive(Debug, Deserialize)]
pub struct ProjectConfig {
    pub name: String,
    #[serde(rename = "type")]
    pub project_type: ProjectType,
    pub auth: Option<AuthProvider>,
    pub email: Option<EmailProvider>,
    #[serde(default)]
    pub resources: Vec<Resource>,
}

#[derive(Debug, Deserialize, PartialEq, Clone)]
#[serde(rename_all = "kebab-case")]
pub enum ProjectType {
    Web,
    ApiWorker,
    LightweightWorker,
    Ios,
    Android,
}

#[derive(Debug, Deserialize, PartialEq, Clone)]
#[serde(rename_all = "kebab-case")]
pub enum AuthProvider {
    Clerk,
    BetterAuth,
    Simple,
}

#[derive(Debug, Deserialize, PartialEq, Clone)]
#[serde(rename_all = "kebab-case")]
pub enum EmailProvider {
    Resend,
}

#[derive(Debug, Deserialize, PartialEq, Clone)]
#[serde(rename_all = "kebab-case")]
pub enum Resource {
    D1,
    R2,
    Kv,
    Queues,
    AiGateway,
}
```

Create placeholder `src/commands/launchpad/validation.rs`:

```rust
use super::config::LaunchpadConfig;
use std::path::Path;

pub fn validate(_config: &LaunchpadConfig, _root: &Path) -> Vec<String> {
    vec![]
}
```

- [ ] **Step 6: Build and verify**

Run: `cargo build 2>&1 | tail -5`

Expected: Build succeeds with no errors.

- [ ] **Step 7: Commit**

```bash
git add src/commands/launchpad/ src/commands/mod.rs src/main.rs Cargo.toml
git commit -m "feat(launchpad): add command skeleton with config parsing"
```

---

### Task 2: Implement config deserialization with full types

**Files:**
- Modify: `src/commands/launchpad/config.rs`

- [ ] **Step 1: Add Serialize derive and display helpers**

Update `src/commands/launchpad/config.rs` to add `Serialize` derives (needed later for template context) and display implementations:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct LaunchpadConfig {
    pub name: String,
    pub root: String,
    pub description: String,
    pub domain: Option<String>,
    pub projects: Vec<ProjectConfig>,
    pub create_resources: bool,
    pub remote: bool,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ProjectConfig {
    pub name: String,
    #[serde(rename = "type")]
    pub project_type: ProjectType,
    pub auth: Option<AuthProvider>,
    pub email: Option<EmailProvider>,
    #[serde(default)]
    pub resources: Vec<Resource>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Clone)]
#[serde(rename_all = "kebab-case")]
pub enum ProjectType {
    Web,
    ApiWorker,
    LightweightWorker,
    Ios,
    Android,
}

impl ProjectType {
    pub fn label(&self) -> &str {
        match self {
            ProjectType::Web => "web app",
            ProjectType::ApiWorker => "API worker",
            ProjectType::LightweightWorker => "lightweight worker",
            ProjectType::Ios => "iOS app",
            ProjectType::Android => "Android app",
        }
    }
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Clone)]
#[serde(rename_all = "kebab-case")]
pub enum AuthProvider {
    Clerk,
    BetterAuth,
    Simple,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Clone)]
#[serde(rename_all = "kebab-case")]
pub enum EmailProvider {
    Resend,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Clone)]
#[serde(rename_all = "kebab-case")]
pub enum Resource {
    D1,
    R2,
    Kv,
    Queues,
    AiGateway,
}

impl Resource {
    pub fn label(&self) -> &str {
        match self {
            Resource::D1 => "D1",
            Resource::R2 => "R2",
            Resource::Kv => "KV",
            Resource::Queues => "Queues",
            Resource::AiGateway => "AI Gateway",
        }
    }
}

impl LaunchpadConfig {
    /// Derive the zone (root domain) from the application domain.
    /// e.g., "app.example.com" -> "example.com", "example.com" -> "example.com"
    pub fn zone(&self) -> Option<String> {
        self.domain.as_ref().map(|d| {
            let parts: Vec<&str> = d.split('.').collect();
            if parts.len() > 2 {
                parts[parts.len() - 2..].join(".")
            } else {
                d.clone()
            }
        })
    }

    pub fn has_api_worker(&self) -> bool {
        self.projects.iter().any(|p| p.project_type == ProjectType::ApiWorker)
    }

    pub fn api_worker_port(&self, ports: &[(String, u16)]) -> Option<u16> {
        self.projects.iter()
            .find(|p| p.project_type == ProjectType::ApiWorker)
            .and_then(|p| ports.iter().find(|(name, _)| name == &p.name).map(|(_, port)| *port))
    }
}

impl ProjectConfig {
    pub fn has_resource(&self, resource: &Resource) -> bool {
        self.resources.contains(resource)
    }

    pub fn is_worker(&self) -> bool {
        matches!(self.project_type, ProjectType::ApiWorker | ProjectType::LightweightWorker)
    }
}
```

- [ ] **Step 2: Build and verify**

Run: `cargo build 2>&1 | tail -5`

Expected: Build succeeds.

- [ ] **Step 3: Commit**

```bash
git add src/commands/launchpad/config.rs
git commit -m "feat(launchpad): add full config types with helpers"
```

---

### Task 3: Implement business logic validation

**Files:**
- Modify: `src/commands/launchpad/validation.rs`

- [ ] **Step 1: Implement all validation rules**

Replace `src/commands/launchpad/validation.rs`:

```rust
use super::config::{LaunchpadConfig, ProjectType, Resource};
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
        // Check for conflicting subdirectories
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

    // Check for duplicate names
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
    // Name validation
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
        ProjectType::ApiWorker => {
            // API workers can have everything — no restrictions
        }
    }
}

fn is_valid_dir_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
}
```

- [ ] **Step 2: Build and verify**

Run: `cargo build 2>&1 | tail -5`

Expected: Build succeeds.

- [ ] **Step 3: Test validation manually**

Create a test config file `/tmp/test-launchpad.json`:

```json
{
  "name": "test-app",
  "root": ".",
  "description": "Test app",
  "projects": [
    {
      "name": "web",
      "type": "web",
      "resources": ["d1"]
    }
  ],
  "create_resources": false,
  "remote": false
}
```

Run: `cargo run -- launchpad --config /tmp/test-launchpad.json`

Expected: Validation error about web project having resources.

- [ ] **Step 4: Test with valid config**

Create `/tmp/test-launchpad-valid.json`:

```json
{
  "name": "test-app",
  "root": "test-app",
  "description": "Test app",
  "domain": "test.groo.bot",
  "projects": [
    {
      "name": "dashboard",
      "type": "web"
    },
    {
      "name": "api",
      "type": "api-worker",
      "resources": ["d1"]
    }
  ],
  "create_resources": false,
  "remote": false
}
```

Run: `cargo run -- launchpad --config /tmp/test-launchpad-valid.json`

Expected: "Config validated. Pipeline not yet implemented."

- [ ] **Step 5: Commit**

```bash
git add src/commands/launchpad/validation.rs
git commit -m "feat(launchpad): add business logic validation with clear error messages"
```

---

### Task 4: Implement state tracking for resume

**Files:**
- Create: `src/commands/launchpad/state.rs`
- Modify: `src/commands/launchpad/mod.rs`

- [ ] **Step 1: Implement the state module**

Create `src/commands/launchpad/state.rs`:

```rust
use anyhow::Result;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize, Deserialize)]
pub struct LaunchpadState {
    pub config_hash: String,
    pub completed_steps: Vec<CompletedStep>,
    pub created_resources: Vec<CreatedResource>,
    #[serde(skip)]
    path: PathBuf,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CompletedStep {
    pub step: String,
    pub project: Option<String>,
    pub result: StepResult,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum StepResult {
    Ok,
    Failed,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CreatedResource {
    #[serde(rename = "type")]
    pub resource_type: String,
    pub name: String,
    pub id: String,
}

impl LaunchpadState {
    pub fn new(config_hash: String, root: &Path) -> Self {
        Self {
            config_hash,
            completed_steps: Vec::new(),
            created_resources: Vec::new(),
            path: root.join(".launchpad-state.json"),
        }
    }

    pub fn load(root: &Path) -> Result<Option<Self>> {
        let path = root.join(".launchpad-state.json");
        if !path.exists() {
            return Ok(None);
        }
        let content = std::fs::read_to_string(&path)?;
        let mut state: Self = serde_json::from_str(&content)?;
        state.path = path;
        Ok(Some(state))
    }

    pub fn save(&self) -> Result<()> {
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(&self.path, content)?;
        Ok(())
    }

    pub fn delete(&self) -> Result<()> {
        if self.path.exists() {
            std::fs::remove_file(&self.path)?;
        }
        Ok(())
    }

    pub fn is_step_complete(&self, step: &str, project: Option<&str>) -> bool {
        self.completed_steps.iter().any(|s| {
            s.step == step
                && s.project.as_deref() == project
                && s.result == StepResult::Ok
        })
    }

    pub fn mark_complete(&mut self, step: &str, project: Option<&str>) {
        self.completed_steps.push(CompletedStep {
            step: step.to_string(),
            project: project.map(|s| s.to_string()),
            result: StepResult::Ok,
            error: None,
        });
    }

    pub fn mark_failed(&mut self, step: &str, project: Option<&str>, error: &str) {
        self.completed_steps.push(CompletedStep {
            step: step.to_string(),
            project: project.map(|s| s.to_string()),
            result: StepResult::Failed,
            error: Some(error.to_string()),
        });
    }

    pub fn add_resource(&mut self, resource_type: &str, name: &str, id: &str) {
        self.created_resources.push(CreatedResource {
            resource_type: resource_type.to_string(),
            name: name.to_string(),
            id: id.to_string(),
        });
    }

    /// Returns true if config has changed since the last run
    pub fn config_changed(&self, new_hash: &str) -> bool {
        self.config_hash != new_hash
    }

    /// Get the index of the first failed step (for resume-after-config-change)
    pub fn first_failure_index(&self) -> Option<usize> {
        self.completed_steps
            .iter()
            .position(|s| s.result == StepResult::Failed)
    }
}

pub fn hash_config(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    format!("{:x}", hasher.finalize())
}
```

- [ ] **Step 2: Add state module to mod.rs**

In `src/commands/launchpad/mod.rs`, add:

```rust
mod state;
```

- [ ] **Step 3: Build and verify**

Run: `cargo build 2>&1 | tail -5`

Expected: Build succeeds. (`sha2` is already a dependency in Cargo.toml.)

- [ ] **Step 4: Commit**

```bash
git add src/commands/launchpad/state.rs src/commands/launchpad/mod.rs
git commit -m "feat(launchpad): add state tracking for resume-on-failure"
```

---

## Chunk 2: Terminal UI

### Task 5: Implement the terminal UI with spinner and collapsing output

**Files:**
- Create: `src/commands/launchpad/ui.rs`
- Modify: `src/commands/launchpad/mod.rs`

- [ ] **Step 1: Implement the UI module**

Create `src/commands/launchpad/ui.rs`:

```rust
use anyhow::Result;
use console::{style, Term};
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::Mutex;

const SPINNER_FRAMES: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
const MAX_LOG_LINES: usize = 5;

pub struct Ui {
    term: Term,
}

impl Ui {
    pub fn new() -> Self {
        Self {
            term: Term::stderr(),
        }
    }

    pub fn header(&self) {
        let _ = self.term.write_line(&format!(
            "\n  {} {}\n",
            "Launchpad",
            style("🚀").dim()
        ));
    }

    pub fn section(&self, title: &str) {
        let _ = self.term.write_line(&format!("  {}", style(title).bold()));
    }

    pub fn success(&self, message: &str) {
        let _ = self.term.write_line(&format!(
            "  {} {}",
            style("✓").green().bold(),
            message
        ));
    }

    pub fn skipped(&self, message: &str) {
        let _ = self.term.write_line(&format!(
            "  {} {} {}",
            style("✓").green().bold(),
            message,
            style("— skipped").dim()
        ));
    }

    pub fn failure(&self, message: &str) {
        let _ = self.term.write_line(&format!(
            "  {} {}",
            style("✗").red().bold(),
            style(message).red()
        ));
    }

    pub fn log_line(&self, line: &str) {
        let _ = self.term.write_line(&format!(
            "    {} {}",
            style(">").dim(),
            style(line).dim()
        ));
    }

    pub fn done(&self) {
        let _ = self.term.write_line(&format!(
            "\n  {} {}\n",
            style("Done!").green().bold(),
            "Run \"groo dev\" to start building."
        ));
    }

    pub fn newline(&self) {
        let _ = self.term.write_line("");
    }

    /// Run a shell command with live spinner and streaming output.
    /// On success: clears log lines, shows checkmark with summary.
    /// On failure: keeps log lines visible, shows error.
    pub async fn run_command(
        &self,
        description: &str,
        command: &str,
        working_dir: &std::path::Path,
    ) -> Result<String> {
        let stop = Arc::new(AtomicBool::new(false));
        let log_lines: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let lines_displayed = Arc::new(Mutex::new(0usize));

        // Start spinner
        let stop_clone = stop.clone();
        let term_clone = self.term.clone();
        let desc = description.to_string();
        let log_lines_clone = log_lines.clone();
        let lines_displayed_clone = lines_displayed.clone();
        let spinner_handle = tokio::spawn(async move {
            let mut frame_idx = 0;
            loop {
                if stop_clone.load(Ordering::Relaxed) {
                    break;
                }

                // Clear previous spinner + log lines
                let displayed = *lines_displayed_clone.lock().await;
                for _ in 0..displayed + 1 {
                    let _ = term_clone.clear_line();
                    let _ = term_clone.move_cursor_up(1);
                }
                let _ = term_clone.clear_line();

                // Write spinner line
                let spinner_char = SPINNER_FRAMES[frame_idx % SPINNER_FRAMES.len()];
                let _ = write!(
                    &term_clone,
                    "  {} {}",
                    style(spinner_char).cyan().bold(),
                    &desc
                );
                let _ = term_clone.write_line("");

                // Write recent log lines
                let lines = log_lines_clone.lock().await;
                let start = if lines.len() > MAX_LOG_LINES {
                    lines.len() - MAX_LOG_LINES
                } else {
                    0
                };
                let visible_lines = &lines[start..];
                for line in visible_lines {
                    let _ = term_clone.write_line(&format!(
                        "    {} {}",
                        style(">").dim(),
                        style(line).dim()
                    ));
                }
                *lines_displayed_clone.lock().await = visible_lines.len();

                frame_idx += 1;
                tokio::time::sleep(Duration::from_millis(80)).await;
            }
        });

        // Write initial spinner line (so first clear has something to clear)
        let _ = self.term.write_line(&format!(
            "  {} {}",
            style(SPINNER_FRAMES[0]).cyan().bold(),
            description
        ));

        // Spawn the process
        let mut child = Command::new("sh")
            .arg("-c")
            .arg(command)
            .current_dir(working_dir)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| anyhow::anyhow!("Failed to run '{}': {}", command, e))?;

        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        let all_output: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

        // Read stdout
        let log_lines_clone = log_lines.clone();
        let all_output_clone = all_output.clone();
        let stdout_handle = tokio::spawn(async move {
            if let Some(stdout) = stdout {
                let reader = BufReader::new(stdout);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    log_lines_clone.lock().await.push(line.clone());
                    all_output_clone.lock().await.push(line);
                }
            }
        });

        // Read stderr
        let log_lines_clone = log_lines.clone();
        let all_output_clone = all_output.clone();
        let stderr_handle = tokio::spawn(async move {
            if let Some(stderr) = stderr {
                let reader = BufReader::new(stderr);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    log_lines_clone.lock().await.push(line.clone());
                    all_output_clone.lock().await.push(line);
                }
            }
        });

        // Wait for process
        let status = child.wait().await?;

        // Wait for readers
        let _ = stdout_handle.await;
        let _ = stderr_handle.await;

        // Stop spinner
        stop.store(true, Ordering::Relaxed);
        let _ = spinner_handle.await;

        // Clear spinner + log lines
        let displayed = *lines_displayed.lock().await;
        for _ in 0..displayed + 1 {
            let _ = self.term.clear_line();
            let _ = self.term.move_cursor_up(1);
        }
        let _ = self.term.clear_line();

        let output = all_output.lock().await.join("\n");

        if status.success() {
            self.success(description);
            Ok(output)
        } else {
            self.failure(description);
            // Show last few lines of output on failure
            let lines = log_lines.lock().await;
            let start = if lines.len() > MAX_LOG_LINES {
                lines.len() - MAX_LOG_LINES
            } else {
                0
            };
            for line in &lines[start..] {
                self.log_line(line);
            }
            anyhow::bail!(
                "Command failed: {}\n{}",
                command,
                output
            );
        }
    }
}
```

- [ ] **Step 2: Add ui module to mod.rs**

In `src/commands/launchpad/mod.rs`, add:

```rust
mod ui;
```

- [ ] **Step 3: Build and verify**

Run: `cargo build 2>&1 | tail -5`

Expected: Build succeeds.

- [ ] **Step 4: Commit**

```bash
git add src/commands/launchpad/ui.rs src/commands/launchpad/mod.rs
git commit -m "feat(launchpad): add terminal UI with spinner and collapsing output"
```

---

## Chunk 3: Scaffolding & Dependencies

### Task 6: Implement project scaffolding

**Files:**
- Create: `src/commands/launchpad/scaffold.rs`
- Modify: `src/commands/launchpad/mod.rs`

- [ ] **Step 1: Implement the scaffold module**

Create `src/commands/launchpad/scaffold.rs`:

```rust
use super::config::ProjectType;
use super::ui::Ui;
use anyhow::Result;
use std::path::Path;

pub async fn scaffold_project(
    ui: &Ui,
    project_name: &str,
    project_type: &ProjectType,
    root: &Path,
) -> Result<()> {
    match project_type {
        ProjectType::Web => {
            ui.run_command(
                &format!("Scaffolded with Vite + React + TypeScript"),
                &format!("npm create vite@latest {} -- --template react-ts", project_name),
                root,
            )
            .await?;
        }
        ProjectType::ApiWorker | ProjectType::LightweightWorker => {
            ui.run_command(
                &format!("Scaffolded with Cloudflare Worker"),
                &format!(
                    "npm create cloudflare@latest {} -- --type hello-world --no-git --no-deploy",
                    project_name
                ),
                root,
            )
            .await?;
        }
        ProjectType::Ios => {
            ui.success(&format!(
                "iOS project '{}' — create via Xcode: File → New → Project → App (SwiftUI)",
                project_name
            ));
        }
        ProjectType::Android => {
            ui.success(&format!(
                "Android project '{}' — create via Android Studio: New Project → Empty Activity (Kotlin)",
                project_name
            ));
        }
    }

    Ok(())
}
```

- [ ] **Step 2: Add scaffold module to mod.rs**

In `src/commands/launchpad/mod.rs`, add:

```rust
mod scaffold;
```

- [ ] **Step 3: Build and verify**

Run: `cargo build 2>&1 | tail -5`

Expected: Build succeeds.

- [ ] **Step 4: Commit**

```bash
git add src/commands/launchpad/scaffold.rs src/commands/launchpad/mod.rs
git commit -m "feat(launchpad): add project scaffolding via npm create"
```

---

### Task 7: Implement dependency installation

**Files:**
- Create: `src/commands/launchpad/deps.rs`
- Modify: `src/commands/launchpad/mod.rs`

- [ ] **Step 1: Implement the deps module**

Create `src/commands/launchpad/deps.rs`:

```rust
use super::config::{AuthProvider, EmailProvider, ProjectConfig, ProjectType};
use super::ui::Ui;
use anyhow::Result;
use std::path::Path;

pub async fn install_deps(ui: &Ui, project: &ProjectConfig, project_dir: &Path) -> Result<()> {
    match project.project_type {
        ProjectType::Web => install_web_deps(ui, project, project_dir).await,
        ProjectType::ApiWorker => install_api_worker_deps(ui, project, project_dir).await,
        ProjectType::LightweightWorker => install_lightweight_worker_deps(ui, project_dir).await,
        ProjectType::Ios | ProjectType::Android => Ok(()), // No npm deps for native
    }
}

async fn install_web_deps(ui: &Ui, project: &ProjectConfig, dir: &Path) -> Result<()> {
    // Core dependencies
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

    // Auth SDK
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

    // Dev dependencies
    let dev_deps = vec![
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

async fn install_api_worker_deps(
    ui: &Ui,
    project: &ProjectConfig,
    dir: &Path,
) -> Result<()> {
    // Core dependencies
    let mut deps = vec!["hono", "drizzle-orm"];

    match &project.auth {
        Some(AuthProvider::Clerk) => deps.push("@clerk/backend"),
        _ => {}
    }

    match &project.email {
        Some(EmailProvider::Resend) => deps.push("resend"),
        _ => {}
    }

    let dep_count = deps.len();
    ui.run_command(
        &format!("Installed {} packages", dep_count),
        &format!("npm install {}", deps.join(" ")),
        dir,
    )
    .await?;

    // Dev dependencies
    let dev_deps = vec!["drizzle-kit", "wrangler", "@types/node"];

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
```

- [ ] **Step 2: Add deps module to mod.rs**

In `src/commands/launchpad/mod.rs`, add:

```rust
mod deps;
```

- [ ] **Step 3: Build and verify**

Run: `cargo build 2>&1 | tail -5`

Expected: Build succeeds.

- [ ] **Step 4: Commit**

```bash
git add src/commands/launchpad/deps.rs src/commands/launchpad/mod.rs
git commit -m "feat(launchpad): add dependency installation per project type"
```

---

### Task 8: Implement port generation

**Files:**
- Create: `src/commands/launchpad/ports.rs`
- Modify: `src/commands/launchpad/mod.rs`

- [ ] **Step 1: Implement the ports module**

Create `src/commands/launchpad/ports.rs`:

```rust
use rand::Rng;
use std::collections::HashSet;

/// Generate `count` unique random ports in the range 10000-65535.
/// Checks that none of the generated ports are currently in use.
pub fn generate_ports(count: usize) -> Vec<u16> {
    let mut rng = rand::thread_rng();
    let mut ports = Vec::with_capacity(count);
    let mut used = HashSet::new();

    for _ in 0..count {
        loop {
            let port: u16 = rng.gen_range(10000..=65535);
            if !used.contains(&port) && !is_port_in_use(port) {
                used.insert(port);
                ports.push(port);
                break;
            }
        }
    }

    ports
}

#[cfg(unix)]
fn is_port_in_use(port: u16) -> bool {
    std::process::Command::new("lsof")
        .args(["-ti", &format!(":{}", port)])
        .output()
        .map(|o| o.status.success() && !o.stdout.is_empty())
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn is_port_in_use(port: u16) -> bool {
    std::net::TcpListener::bind(("127.0.0.1", port)).is_err()
}
```

- [ ] **Step 2: Add ports module to mod.rs**

In `src/commands/launchpad/mod.rs`, add:

```rust
mod ports;
```

- [ ] **Step 3: Build and verify**

Run: `cargo build 2>&1 | tail -5`

Expected: Build succeeds.

- [ ] **Step 4: Commit**

```bash
git add src/commands/launchpad/ports.rs src/commands/launchpad/mod.rs
git commit -m "feat(launchpad): add unique port generation"
```

---

## Chunk 4: Templates

### Task 9: Create all Tera template files

**Files:**
- Create: all files under `templates/launchpad/`

- [ ] **Step 1: Create templates directory**

Run: `mkdir -p templates/launchpad`

- [ ] **Step 2: Create wrangler.jsonc template**

Create `templates/launchpad/wrangler.jsonc.tera`:

```
{
  "$schema": "node_modules/wrangler/config-schema.json",
  "name": "{{ prefix }}-{{ project_name }}",
  "main": "src/index.ts",
  "compatibility_date": "{{ today }}",
  "observability": { "enabled": true },
  "upload_source_maps": true,
  "compatibility_flags": ["nodejs_compat"],
  "dev": { "port": {{ port }} }{% if project_type == "api-worker" and domain %},
  "routes": [
    {
      "pattern": "{{ domain }}/v1/*",
      "zone_name": "{{ zone }}"
    }
  ]{% endif %}{% if has_d1 %},
  "d1_databases": [
    {
      "binding": "DB",
      "database_name": "{{ prefix }}-d1",
      "database_id": "{{ d1_id | default(value='') }}"{% if remote %},
      "remote": true{% endif %}
    }
  ]{% endif %}{% if has_r2 %},
  "r2_buckets": [
    {
      "binding": "BUCKET",
      "bucket_name": "{{ prefix }}-r2"{% if remote %},
      "remote": true{% endif %}
    }
  ]{% endif %}{% if has_kv %},
  "kv_namespaces": [
    {
      "binding": "KV",
      "id": "{{ kv_id | default(value='') }}"{% if remote %},
      "remote": true{% endif %}
    }
  ]{% endif %}{% if has_queues %},
  "queues": {
    "producers": [
      {
        "binding": "QUEUE",
        "queue": "{{ prefix }}-queue"
      }
    ]
  }{% endif %}{% if has_ai_gateway %},
  "ai": {
    "binding": "AI"
  }{% endif %}
}
```

- [ ] **Step 3: Create vite.config.ts template**

Create `templates/launchpad/vite.config.ts.tera`:

```
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

export default defineConfig({
  plugins: [react(), tailwindcss()],
  server: {
    port: {{ port }},{% if api_port %}
    proxy: {
      "/v1": {
        target: "http://localhost:{{ api_port }}",
        changeOrigin: true,
      },
    },{% endif %}
  },
});
```

- [ ] **Step 4: Create drizzle.config.ts template**

Create `templates/launchpad/drizzle.config.ts.tera`:

```
import { defineConfig } from "drizzle-kit";

export default defineConfig({
  schema: "./src/db/schema.ts",
  out: "./migrations",
  dialect: "sqlite",
});
```

- [ ] **Step 5: Create hono-entry.ts template**

Create `templates/launchpad/hono-entry.ts.tera`:

```
import { Hono } from "hono";

const app = new Hono<{ Bindings: CloudflareBindings }>().basePath("/v1");

app.get("/health", (c) =>
  c.json({ status: "ok", version: c.env.VERSION })
);

export default app;
```

- [ ] **Step 6: Create axios-client.ts template**

Create `templates/launchpad/axios-client.ts.tera`:

```
import axios from "axios";

export const api = axios.create({
  baseURL: "/v1",
});
```

- [ ] **Step 7: Create config-worker.ts template**

Create `templates/launchpad/config-worker.ts.tera`:

```
export function getConfig(env: CloudflareBindings) {
  return {
    get version(): string {
      return env.VERSION || "dev";
    },{% if auth == "clerk" %}
    get clerkSecretKey(): string {
      if (!env.CLERK_SECRET_KEY) throw new Error("CLERK_SECRET_KEY is not set in .dev.vars");
      return env.CLERK_SECRET_KEY;
    },{% endif %}{% if email == "resend" %}
    get resendApiKey(): string {
      if (!env.RESEND_API_KEY) throw new Error("RESEND_API_KEY is not set in .dev.vars");
      return env.RESEND_API_KEY;
    },{% endif %}
  };
}
```

- [ ] **Step 8: Create config-web.ts template**

Create `templates/launchpad/config-web.ts.tera`:

```
export const config = {
  get version(): string {
    return import.meta.env.VITE_VERSION || "dev";
  },{% if auth == "clerk" %}
  get clerkPublishableKey(): string {
    const val = import.meta.env.VITE_CLERK_PUBLISHABLE_KEY;
    if (!val) throw new Error("VITE_CLERK_PUBLISHABLE_KEY is not set in .env");
    return val;
  },{% endif %}
};
```

- [ ] **Step 9: Create schema.ts template**

Create `templates/launchpad/schema.ts.tera`:

```
import { sqliteTable, text, integer } from "drizzle-orm/sqlite-core";

export const example = sqliteTable("example", {
  id: integer("id").primaryKey({ autoIncrement: true }),
  name: text("name").notNull(),
  createdAt: integer("created_at", { mode: "timestamp" })
    .notNull()
    .$defaultFn(() => new Date()),
});
```

- [ ] **Step 10: Create env example templates**

Create `templates/launchpad/env.example.tera`:

```
VITE_VERSION=dev{% if auth == "clerk" %}
VITE_CLERK_PUBLISHABLE_KEY=pk_test_xxxx{% endif %}
```

Create `templates/launchpad/dev.vars.example.tera`:

```
VERSION=dev{% if auth == "clerk" %}
CLERK_SECRET_KEY=sk_test_xxxx{% endif %}{% if email == "resend" %}
RESEND_API_KEY=re_xxxx{% endif %}
```

- [ ] **Step 11: Create GitHub Actions templates**

Create `templates/launchpad/deploy-worker.yml.tera`:

```
name: Deploy {{ project_name }}

on:
  push:
    branches:
      - main
    paths:
      - "{{ project_dir }}/**"
      - ".github/workflows/deploy-{{ project_name }}.yml"
  workflow_dispatch:

permissions:
  contents: write

jobs:
  deploy:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Record Release
        id: release
        uses: groo-dev/record-release@v1
        with:
          token: {% raw %}${{ secrets.OPS_API_TOKEN }}{% endraw %}
          environment: production
          bump: patch

      - name: Setup Node.js
        uses: actions/setup-node@v4
        with:
          node-version: "24"
          cache: "npm"
          cache-dependency-path: "{{ project_dir }}/package-lock.json"

      - name: Install dependencies
        run: npm ci
        working-directory: "{{ project_dir }}"

      - name: Deploy
        run: npm run deploy
        working-directory: "{{ project_dir }}"
        env:
          CLOUDFLARE_API_TOKEN: {% raw %}${{ secrets.CLOUDFLARE_API_TOKEN }}{% endraw %}
          CLOUDFLARE_ACCOUNT_ID: {% raw %}${{ secrets.CLOUDFLARE_ACCOUNT_ID }}{% endraw %}
          VERSION: {% raw %}${{ steps.release.outputs.version }}{% endraw %}
```

Create `templates/launchpad/deploy-web.yml.tera`:

```
name: Deploy {{ project_name }}

on:
  push:
    branches:
      - main
    paths:
      - "{{ project_dir }}/**"
      - ".github/workflows/deploy-{{ project_name }}.yml"
  workflow_dispatch:

permissions:
  contents: write

jobs:
  deploy:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Record Release
        id: release
        uses: groo-dev/record-release@v1
        with:
          token: {% raw %}${{ secrets.OPS_API_TOKEN }}{% endraw %}
          environment: production
          bump: patch

      - name: Setup Node.js
        uses: actions/setup-node@v4
        with:
          node-version: "24"
          cache: "npm"
          cache-dependency-path: "{{ project_dir }}/package-lock.json"

      - name: Install dependencies
        run: npm ci
        working-directory: "{{ project_dir }}"

      - name: Deploy
        run: npm run deploy
        working-directory: "{{ project_dir }}"
        env:
          CLOUDFLARE_API_TOKEN: {% raw %}${{ secrets.CLOUDFLARE_API_TOKEN }}{% endraw %}
          CLOUDFLARE_ACCOUNT_ID: {% raw %}${{ secrets.CLOUDFLARE_ACCOUNT_ID }}{% endraw %}
          VERSION: {% raw %}${{ steps.release.outputs.version }}{% endraw %}
```

- [ ] **Step 12: Create gitignore template**

Create `templates/launchpad/gitignore.tera`:

```
node_modules/
dist/
.dev.vars
.env
.wrangler/
*.log
.DS_Store
.playwright-mcp/
.launchpad-state.json
```

- [ ] **Step 13: Create CLAUDE.md template**

Create `templates/launchpad/claude.md.tera`:

```
# {{ name }}

{{ description }}

## Coding Practices

### Error Handling
- Never write code in huge try/catch blocks. Catch errors at the specific
  operation that can fail. Each catch must log the error with full context.
- Never suppress errors or build fallbacks that hide problems. Log everything,
  let errors surface.
- If the correct approach fails, throw an error. Don't patch around it with
  fallback solutions — debug and fix the root cause.

### Environment Variables & Configuration
- Never use default values where a value is expected. If an env var, function
  parameter, or API param is required and missing, throw an error immediately.
- Never create types for env vars manually. Run `npm run cf-typegen` to generate
  types from wrangler.jsonc. Add secrets to `.dev.vars`, then run cf-typegen.
- Access all env vars through config.ts getters that validate and throw on missing.
- Vars go in wrangler.jsonc. Secrets go in .dev.vars. Never set defaults in code.

### Database Migrations
- Never manually create or edit migration files in `migrations/` directories.
- Generate migrations from Drizzle schema: `npm run db:generate`
- Apply with wrangler: `npm run db:migrate:local` / `npm run db:migrate:remote`

### UI Design
- Always use the frontend-design skill to design user interfaces.

### Planning
- For complex or multi-step tasks, use the brainstorming skill first.
{% if has_ios %}
### SwiftUI
- After writing SwiftUI code, use swiftui-pro skill to review before committing.
  Show the review to the developer and let them choose what to fix.
{% endif %}
## Project Structure

```
{% for project in projects %}{{ project.dir }}/  — {{ project.description }}
{% endfor %}```

## Development

```bash
groo dev
```

## Deployment

Auto-deploys on push to `main` via GitHub Actions.

Manual deploy per project:
```bash
cd <project> && VERSION=x.x.x npm run deploy
```
```

- [ ] **Step 14: Create README.md template**

Create `templates/launchpad/readme.md.tera`:

```
# {{ name }}

{{ description }}

## Project Structure

{% for project in projects %}- **{{ project.dir }}/** — {{ project.description }}
{% endfor %}
## Prerequisites

- Node.js (v24+)
- [wrangler](https://developers.cloudflare.com/workers/wrangler/) (`npm install -g wrangler`)
- [groo CLI](https://github.com/groo-dev/cli) (`brew install groo-dev/tap/groo`)

## Setup

```bash
git clone <repo-url>
cd {{ name }}
{% for project in projects %}
# {{ project.dir }}
cd {{ project.dir }} && npm install && cd ..{% endfor %}
```

Copy environment files:
{% for project in projects %}{% if project.env_example %}
```bash
cp {{ project.dir }}/{{ project.env_file }} {{ project.dir }}/{{ project.env_target }}
```
{% endif %}{% endfor %}
## Run Locally

```bash
groo dev
```

## Deployment

Auto-deploys on push to `main` via GitHub Actions.

Required GitHub secrets:
- `CLOUDFLARE_API_TOKEN` — Cloudflare API token
- `CLOUDFLARE_ACCOUNT_ID` — Cloudflare account ID
- `OPS_API_TOKEN` — Groo Ops Dashboard token

Manual deploy:
```bash
cd <project> && VERSION=x.x.x npm run deploy
```
{% if has_env_vars %}
## Environment Variables

| Variable | Project | Where |
|---|---|---|{% for env_var in env_vars %}
| `{{ env_var.name }}` | {{ env_var.project }} | {{ env_var.location }} |{% endfor %}
{% endif %}
```

- [ ] **Step 15: Create TODO.md template**

Create `templates/launchpad/todo.md.tera`:

```
# Setup TODO

## Checklist

- [ ] Create Cloudflare API token
- [ ] Add GitHub secrets (`CLOUDFLARE_API_TOKEN`, `CLOUDFLARE_ACCOUNT_ID`, `OPS_API_TOKEN`){% if domain %}
- [ ] Configure custom domain in Cloudflare{% endif %}
- [ ] Add production environment variables

---

## Create Cloudflare API token

1. Go to https://dash.cloudflare.com/profile/api-tokens
2. Click "Create Token"
3. Use the "Edit Cloudflare Workers" template
4. Add `Cloudflare Pages: Edit` permission
5. Set zone resources to your domain
6. Create and copy the token

## Add GitHub secrets

Go to your GitHub repo > Settings > Secrets and variables > Actions > New repository secret.

| Secret | Where to find it |
|--------|-----------------|
| `CLOUDFLARE_API_TOKEN` | The token you just created above |
| `CLOUDFLARE_ACCOUNT_ID` | Cloudflare dashboard > any domain > Overview > right sidebar |
| `OPS_API_TOKEN` | Groo Ops Dashboard > Project Settings > API Tokens |
{% if domain %}
## Configure custom domain

1. Go to Cloudflare dashboard > your zone > DNS
2. Add a CNAME record for `{{ domain }}` pointing to `{{ prefix }}-web.pages.dev` (proxied)
3. Go to Pages > {{ prefix }}-web > Custom domains > Add `{{ domain }}`{% if has_api_worker %}
4. The API route (`{{ domain }}/v1/*`) is configured in wrangler.jsonc and activates on first deploy{% endif %}
{% endif %}
## Add production environment variables

For each worker, add secrets via wrangler CLI:

```bash
{% for project in worker_projects %}wrangler secret put SECRET_NAME --name {{ prefix }}-{{ project.name }}
{% endfor %}```

Or add them in Cloudflare dashboard > Workers & Pages > your worker > Settings > Variables.
```

- [ ] **Step 16: Create settings.local.json template**

Create `templates/launchpad/settings.local.json.tera`:

```
{
  "permissions": {
    "deny": [{% for project in d1_projects %}
      "Edit({{ project.dir }}/migrations/**)",
      "Write({{ project.dir }}/migrations/**)"{% if not loop.last %},{% endif %}{% endfor %}
    ]
  },
  "attribution": {
    "commit": "",
    "pr": ""
  }
}
```

- [ ] **Step 17: Commit all templates**

```bash
git add templates/launchpad/
git commit -m "feat(launchpad): add all Tera template files"
```

---

### Task 10: Implement template rendering engine

**Files:**
- Create: `src/commands/launchpad/templates.rs`
- Modify: `src/commands/launchpad/mod.rs`

- [ ] **Step 1: Implement the templates module**

Create `src/commands/launchpad/templates.rs`:

```rust
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

    /// Build context for a worker's wrangler.jsonc
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

    /// Build context for a web project's vite.config.ts
    pub fn vite_context(&self, port: u16, api_port: Option<u16>) -> Context {
        let mut ctx = Context::new();
        ctx.insert("port", &port);
        ctx.insert("api_port", &api_port);
        ctx
    }

    /// Build context for a worker's config.ts
    pub fn worker_config_context(&self, project: &ProjectConfig) -> Context {
        let mut ctx = Context::new();
        ctx.insert(
            "auth",
            &project
                .auth
                .as_ref()
                .map(|a| serde_json::to_value(a).unwrap()),
        );
        ctx.insert(
            "email",
            &project
                .email
                .as_ref()
                .map(|e| serde_json::to_value(e).unwrap()),
        );
        ctx
    }

    /// Build context for a web project's config.ts
    pub fn web_config_context(&self, project: &ProjectConfig) -> Context {
        let mut ctx = Context::new();
        ctx.insert(
            "auth",
            &project
                .auth
                .as_ref()
                .map(|a| serde_json::to_value(a).unwrap()),
        );
        ctx
    }

    /// Build context for env example files
    pub fn env_example_context(&self, project: &ProjectConfig) -> Context {
        let mut ctx = Context::new();
        ctx.insert(
            "auth",
            &project
                .auth
                .as_ref()
                .map(|a| serde_json::to_value(a).unwrap()),
        );
        ctx.insert(
            "email",
            &project
                .email
                .as_ref()
                .map(|e| serde_json::to_value(e).unwrap()),
        );
        ctx
    }

    /// Build context for deploy workflow
    pub fn deploy_context(&self, project_name: &str, project_dir: &str) -> Context {
        let mut ctx = Context::new();
        ctx.insert("project_name", project_name);
        ctx.insert("project_dir", project_dir);
        ctx
    }

    /// Build context for project-level files (CLAUDE.md, README, TODO)
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
            &config
                .projects
                .iter()
                .any(|p| p.project_type == ProjectType::Ios),
        );

        // Build project descriptions for README/CLAUDE.md
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

        // Worker projects for TODO
        let worker_projects: Vec<serde_json::Value> = config
            .projects
            .iter()
            .filter(|p| p.is_worker())
            .map(|p| serde_json::json!({ "name": p.name }))
            .collect();
        ctx.insert("worker_projects", &worker_projects);

        // D1 projects for settings.local.json
        let d1_projects: Vec<serde_json::Value> = config
            .projects
            .iter()
            .filter(|p| p.has_resource(&Resource::D1))
            .map(|p| serde_json::json!({ "dir": p.name }))
            .collect();
        ctx.insert("d1_projects", &d1_projects);

        // Environment variables table for README
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

/// Write rendered template to a file, creating parent directories as needed.
pub fn write_template(content: &str, path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, content)?;
    Ok(())
}
```

- [ ] **Step 2: Add templates module to mod.rs**

In `src/commands/launchpad/mod.rs`, add:

```rust
mod templates;
```

- [ ] **Step 3: Build and verify**

Run: `cargo build 2>&1 | tail -5`

Expected: Build succeeds. If there are template syntax errors, fix them based on Tera's error messages.

- [ ] **Step 4: Commit**

```bash
git add src/commands/launchpad/templates.rs src/commands/launchpad/mod.rs
git commit -m "feat(launchpad): add template rendering engine with embedded templates"
```

---

## Chunk 5: Resource Creation & Cleanup

### Task 11: Implement Cloudflare resource creation

**Files:**
- Create: `src/commands/launchpad/resources.rs`
- Modify: `src/commands/launchpad/mod.rs`

- [ ] **Step 1: Implement the resources module**

Create `src/commands/launchpad/resources.rs`:

```rust
use super::config::{LaunchpadConfig, ProjectConfig, ProjectType, Resource};
use super::state::LaunchpadState;
use super::ui::Ui;
use anyhow::Result;
use std::path::Path;

pub async fn create_resources(
    ui: &Ui,
    config: &LaunchpadConfig,
    state: &mut LaunchpadState,
    root: &Path,
) -> Result<()> {
    // Create Pages project for web projects
    for project in &config.projects {
        if project.project_type == ProjectType::Web {
            let pages_name = format!("{}-web", config.name);
            let output = ui
                .run_command(
                    &format!("Created Pages project \"{}\"", pages_name),
                    &format!("wrangler pages project create {}", pages_name),
                    root,
                )
                .await?;
            state.add_resource("pages", &pages_name, "");
            state.save()?;
        }
    }

    // Create resources for each worker
    for project in &config.projects {
        if !project.is_worker() {
            continue;
        }

        let project_dir = root.join(&project.name);

        for resource in &project.resources {
            match resource {
                Resource::D1 => {
                    let name = format!("{}-d1", config.name);
                    let output = ui
                        .run_command(
                            &format!("Created D1 database \"{}\"", name),
                            &format!("wrangler d1 create {}", name),
                            &project_dir,
                        )
                        .await?;
                    // Parse database ID from output
                    let id = parse_d1_id(&output).unwrap_or_default();
                    state.add_resource("d1", &name, &id);
                    state.save()?;
                }
                Resource::R2 => {
                    let name = format!("{}-r2", config.name);
                    ui.run_command(
                        &format!("Created R2 bucket \"{}\"", name),
                        &format!("wrangler r2 bucket create {}", name),
                        &project_dir,
                    )
                    .await?;
                    state.add_resource("r2", &name, "");
                    state.save()?;
                }
                Resource::Kv => {
                    let name = format!("{}-kv", config.name);
                    let output = ui
                        .run_command(
                            &format!("Created KV namespace \"{}\"", name),
                            &format!("wrangler kv namespace create {}", name),
                            &project_dir,
                        )
                        .await?;
                    let id = parse_kv_id(&output).unwrap_or_default();
                    state.add_resource("kv", &name, &id);
                    state.save()?;
                }
                Resource::Queues => {
                    let name = format!("{}-queue", config.name);
                    ui.run_command(
                        &format!("Created Queue \"{}\"", name),
                        &format!("wrangler queues create {}", name),
                        &project_dir,
                    )
                    .await?;
                    state.add_resource("queues", &name, "");
                    state.save()?;
                }
                Resource::AiGateway => {
                    // AI Gateway is configured in wrangler.jsonc only, no CLI creation needed
                }
            }
        }
    }

    Ok(())
}

/// Parse D1 database ID from wrangler output.
/// Output format: "Created database 'name' at location\ndatabase_id = \"abc-123\""
fn parse_d1_id(output: &str) -> Option<String> {
    for line in output.lines() {
        if let Some(id_part) = line.strip_prefix("database_id = ") {
            return Some(id_part.trim().trim_matches('"').to_string());
        }
        // Also try: | database_id | abc-123 |
        if line.contains("database_id") && line.contains('|') {
            let parts: Vec<&str> = line.split('|').collect();
            if parts.len() >= 3 {
                return Some(parts[2].trim().to_string());
            }
        }
    }
    None
}

/// Parse KV namespace ID from wrangler output.
/// Output format: "Add the following to your configuration file...\nid = \"abc-123\""
fn parse_kv_id(output: &str) -> Option<String> {
    for line in output.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("id = ") || trimmed.starts_with("\"id\": ") {
            return Some(
                trimmed
                    .split('=')
                    .last()
                    .or_else(|| trimmed.split(':').last())
                    .unwrap_or("")
                    .trim()
                    .trim_matches('"')
                    .trim_matches(',')
                    .to_string(),
            );
        }
    }
    None
}

/// Delete Cloudflare resources tracked in state (for --clean)
pub async fn delete_resources(ui: &Ui, state: &LaunchpadState, root: &Path) -> Result<()> {
    for resource in &state.created_resources {
        match resource.resource_type.as_str() {
            "d1" => {
                let _ = ui
                    .run_command(
                        &format!("Deleted D1 database \"{}\"", resource.name),
                        &format!("wrangler d1 delete {} --yes", resource.name),
                        root,
                    )
                    .await;
            }
            "r2" => {
                let _ = ui
                    .run_command(
                        &format!("Deleted R2 bucket \"{}\"", resource.name),
                        &format!("wrangler r2 bucket delete {}", resource.name),
                        root,
                    )
                    .await;
            }
            "kv" => {
                if !resource.id.is_empty() {
                    let _ = ui
                        .run_command(
                            &format!("Deleted KV namespace \"{}\"", resource.name),
                            &format!("wrangler kv namespace delete --namespace-id {}", resource.id),
                            root,
                        )
                        .await;
                }
            }
            "queues" => {
                let _ = ui
                    .run_command(
                        &format!("Deleted Queue \"{}\"", resource.name),
                        &format!("wrangler queues delete {}", resource.name),
                        root,
                    )
                    .await;
            }
            "pages" => {
                let _ = ui
                    .run_command(
                        &format!("Deleted Pages project \"{}\"", resource.name),
                        &format!("wrangler pages project delete {} --yes", resource.name),
                        root,
                    )
                    .await;
            }
            _ => {}
        }
    }
    Ok(())
}
```

- [ ] **Step 2: Add resources module to mod.rs**

In `src/commands/launchpad/mod.rs`, add:

```rust
mod resources;
```

- [ ] **Step 3: Build and verify**

Run: `cargo build 2>&1 | tail -5`

Expected: Build succeeds.

- [ ] **Step 4: Commit**

```bash
git add src/commands/launchpad/resources.rs src/commands/launchpad/mod.rs
git commit -m "feat(launchpad): add Cloudflare resource creation and deletion"
```

---

### Task 12: Implement clean command

**Files:**
- Create: `src/commands/launchpad/clean.rs`
- Modify: `src/commands/launchpad/mod.rs`

- [ ] **Step 1: Implement the clean module**

Create `src/commands/launchpad/clean.rs`:

```rust
use super::config::LaunchpadConfig;
use super::resources;
use super::state::LaunchpadState;
use super::ui::Ui;
use anyhow::Result;
use std::path::Path;

pub async fn clean_previous_run(
    ui: &Ui,
    config: &LaunchpadConfig,
    state: &LaunchpadState,
    root: &Path,
) -> Result<()> {
    ui.section("Cleaning previous run...");

    // Delete project directories
    for project in &config.projects {
        let project_dir = root.join(&project.name);
        if project_dir.exists() {
            std::fs::remove_dir_all(&project_dir)?;
            ui.success(&format!("Removed {}/", project.name));
        }
    }

    // Delete Cloudflare resources
    if !state.created_resources.is_empty() {
        resources::delete_resources(ui, state, root).await?;
    }

    // Delete generated project files
    let files_to_clean = [
        "CLAUDE.md",
        "README.md",
        "TODO.md",
        ".gitignore",
        ".claude/settings.local.json",
    ];
    for file in &files_to_clean {
        let path = root.join(file);
        if path.exists() {
            std::fs::remove_file(&path)?;
        }
    }

    // Delete .github/workflows
    let workflows_dir = root.join(".github/workflows");
    if workflows_dir.exists() {
        std::fs::remove_dir_all(&workflows_dir)?;
    }

    // Delete .git if we created it
    let git_dir = root.join(".git");
    if git_dir.exists() {
        std::fs::remove_dir_all(&git_dir)?;
    }

    // Delete state file
    state.delete()?;

    ui.newline();
    ui.section("Starting fresh...");
    ui.newline();

    Ok(())
}
```

- [ ] **Step 2: Add clean module to mod.rs**

In `src/commands/launchpad/mod.rs`, add:

```rust
mod clean;
```

- [ ] **Step 3: Build and verify**

Run: `cargo build 2>&1 | tail -5`

Expected: Build succeeds.

- [ ] **Step 4: Commit**

```bash
git add src/commands/launchpad/clean.rs src/commands/launchpad/mod.rs
git commit -m "feat(launchpad): add clean command for previous failed runs"
```

---

## Chunk 6: Pipeline Orchestration

### Task 13: Implement the full execution pipeline

**Files:**
- Create: `src/commands/launchpad/pipeline.rs`
- Modify: `src/commands/launchpad/mod.rs`

- [ ] **Step 1: Implement the pipeline module**

Create `src/commands/launchpad/pipeline.rs`:

```rust
use super::config::{LaunchpadConfig, ProjectType, Resource};
use super::deps;
use super::ports;
use super::resources;
use super::scaffold;
use super::state::{self, LaunchpadState};
use super::templates::{self, TemplateEngine};
use super::ui::Ui;
use anyhow::Result;
use std::path::Path;

pub async fn run_pipeline(
    config: &LaunchpadConfig,
    config_content: &str,
    root: &Path,
    state: &mut LaunchpadState,
    ui: &Ui,
) -> Result<()> {
    let engine = TemplateEngine::new()?;
    let is_resuming = !state.completed_steps.is_empty();

    if is_resuming {
        ui.section("Resuming from previous run...");
        ui.newline();
    }

    // Step 2: Create root directory
    if config.root != "." && !root.exists() {
        std::fs::create_dir_all(root)?;
        ui.success(&format!("Created directory {}/", config.root));
    }

    // Step 3: Scaffold and install deps for each project
    for project in &config.projects {
        let project_dir = root.join(&project.name);

        ui.section(&format!(
            "Creating {} \"{}\"",
            project.project_type.label(),
            project.name
        ));

        // 3a: Scaffold
        if state.is_step_complete("scaffold", Some(&project.name)) {
            ui.skipped("Scaffold");
        } else {
            scaffold::scaffold_project(ui, &project.name, &project.project_type, root).await?;
            state.mark_complete("scaffold", Some(&project.name));
            state.save()?;
        }

        // 3b: Install dependencies
        if state.is_step_complete("install_deps", Some(&project.name)) {
            ui.skipped("Dependencies");
        } else {
            deps::install_deps(ui, project, &project_dir).await?;
            state.mark_complete("install_deps", Some(&project.name));
            state.save()?;
        }

        ui.newline();
    }

    // Step 4: Generate ports
    let port_projects: Vec<&str> = config
        .projects
        .iter()
        .filter(|p| !matches!(p.project_type, ProjectType::Ios | ProjectType::Android))
        .map(|p| p.name.as_str())
        .collect();
    let port_values = ports::generate_ports(port_projects.len());
    let port_map: Vec<(String, u16)> = port_projects
        .iter()
        .zip(port_values.iter())
        .map(|(name, port)| (name.to_string(), *port))
        .collect();

    let api_port = config.api_worker_port(&port_map);

    // Step 5: Write config files
    if state.is_step_complete("write_configs", None) {
        ui.skipped("Config files");
    } else {
        ui.section("Writing config files");

        for project in &config.projects {
            let project_dir = root.join(&project.name);

            if let Some((_, port)) = port_map.iter().find(|(n, _)| n == &project.name) {
                match project.project_type {
                    ProjectType::ApiWorker | ProjectType::LightweightWorker => {
                        // wrangler.jsonc
                        let ctx = engine.wrangler_context(config, project, *port);
                        let content = engine.render("wrangler.jsonc", &ctx)?;
                        templates::write_template(&content, &project_dir.join("wrangler.jsonc"))?;
                        ui.success(&format!(
                            "{}/wrangler.jsonc — port {}",
                            project.name, port
                        ));

                        // drizzle.config.ts (if D1)
                        if project.has_resource(&Resource::D1) {
                            let ctx = tera::Context::new();
                            let content = engine.render("drizzle.config.ts", &ctx)?;
                            templates::write_template(
                                &content,
                                &project_dir.join("drizzle.config.ts"),
                            )?;
                            ui.success(&format!("{}/drizzle.config.ts", project.name));
                        }
                    }
                    ProjectType::Web => {
                        // vite.config.ts
                        let ctx = engine.vite_context(*port, api_port);
                        let content = engine.render("vite.config.ts", &ctx)?;
                        templates::write_template(
                            &content,
                            &project_dir.join("vite.config.ts"),
                        )?;
                        ui.success(&format!(
                            "{}/vite.config.ts — port {}{}",
                            project.name,
                            port,
                            if let Some(ap) = api_port {
                                format!(", proxy → :{}", ap)
                            } else {
                                String::new()
                            }
                        ));
                    }
                    _ => {}
                }
            }
        }

        state.mark_complete("write_configs", None);
        state.save()?;
        ui.newline();
    }

    // Step 6: Write package.json scripts
    if state.is_step_complete("write_scripts", None) {
        ui.skipped("Package.json scripts");
    } else {
        for project in &config.projects {
            let project_dir = root.join(&project.name);
            let pkg_path = project_dir.join("package.json");

            if pkg_path.exists() {
                let pkg_content = std::fs::read_to_string(&pkg_path)?;
                let mut pkg: serde_json::Value = serde_json::from_str(&pkg_content)?;

                if let Some(scripts) = pkg.get_mut("scripts").and_then(|s| s.as_object_mut()) {
                    match project.project_type {
                        ProjectType::ApiWorker | ProjectType::LightweightWorker => {
                            scripts.insert(
                                "dev".to_string(),
                                serde_json::json!("wrangler dev"),
                            );
                            scripts.insert(
                                "deploy".to_string(),
                                serde_json::json!("wrangler deploy --minify --var VERSION:$VERSION"),
                            );
                            scripts.insert(
                                "cf-typegen".to_string(),
                                serde_json::json!("wrangler types --env-interface CloudflareBindings"),
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
                            scripts.insert("dev".to_string(), serde_json::json!("vite"));
                            scripts.insert(
                                "build".to_string(),
                                serde_json::json!("tsc -b && vite build"),
                            );
                            scripts.insert(
                                "preview".to_string(),
                                serde_json::json!("vite preview"),
                            );
                            scripts.insert("lint".to_string(), serde_json::json!("eslint ."));
                            scripts.insert(
                                "deploy".to_string(),
                                serde_json::json!(format!(
                                    "VITE_VERSION=$VERSION npm run build && wrangler pages deploy dist --project-name {}-web",
                                    config.name
                                )),
                            );
                        }
                        _ => {}
                    }
                }

                std::fs::write(&pkg_path, serde_json::to_string_pretty(&pkg)?)?;
            }
        }

        state.mark_complete("write_scripts", None);
        state.save()?;
    }

    // Step 7: Write boilerplate code
    if state.is_step_complete("write_boilerplate", None) {
        ui.skipped("Boilerplate code");
    } else {
        ui.section("Writing boilerplate code");

        for project in &config.projects {
            let project_dir = root.join(&project.name);

            match project.project_type {
                ProjectType::ApiWorker => {
                    // Hono entry point
                    let ctx = tera::Context::new();
                    let content = engine.render("hono-entry.ts", &ctx)?;
                    templates::write_template(&content, &project_dir.join("src/index.ts"))?;
                    ui.success(&format!("{}/src/index.ts — Hono with /v1 base path", project.name));

                    // Worker config.ts
                    let ctx = engine.worker_config_context(project);
                    let content = engine.render("config-worker.ts", &ctx)?;
                    templates::write_template(&content, &project_dir.join("src/config.ts"))?;
                    ui.success(&format!("{}/src/config.ts", project.name));

                    // Drizzle schema (if D1)
                    if project.has_resource(&Resource::D1) {
                        let ctx = tera::Context::new();
                        let content = engine.render("schema.ts", &ctx)?;
                        templates::write_template(
                            &content,
                            &project_dir.join("src/db/schema.ts"),
                        )?;
                        ui.success(&format!("{}/src/db/schema.ts", project.name));
                    }
                }
                ProjectType::LightweightWorker => {
                    // Worker config.ts
                    let ctx = engine.worker_config_context(project);
                    let content = engine.render("config-worker.ts", &ctx)?;
                    templates::write_template(&content, &project_dir.join("src/config.ts"))?;
                    ui.success(&format!("{}/src/config.ts", project.name));
                }
                ProjectType::Web => {
                    // Axios API client (only if there's an API worker)
                    if config.has_api_worker() {
                        let ctx = tera::Context::new();
                        let content = engine.render("axios-client.ts", &ctx)?;
                        templates::write_template(
                            &content,
                            &project_dir.join("src/lib/api.ts"),
                        )?;
                        ui.success(&format!("{}/src/lib/api.ts", project.name));
                    }

                    // Web config.ts
                    let ctx = engine.web_config_context(project);
                    let content = engine.render("config-web.ts", &ctx)?;
                    templates::write_template(&content, &project_dir.join("src/config.ts"))?;
                    ui.success(&format!("{}/src/config.ts", project.name));
                }
                _ => {}
            }
        }

        state.mark_complete("write_boilerplate", None);
        state.save()?;
        ui.newline();
    }

    // Step 8: Write env example files
    if state.is_step_complete("write_env_examples", None) {
        ui.skipped("Environment examples");
    } else {
        for project in &config.projects {
            let project_dir = root.join(&project.name);
            let ctx = engine.env_example_context(project);

            match project.project_type {
                ProjectType::Web => {
                    let content = engine.render("env.example", &ctx)?;
                    templates::write_template(&content, &project_dir.join(".env.example"))?;
                }
                ProjectType::ApiWorker | ProjectType::LightweightWorker => {
                    let content = engine.render("dev.vars.example", &ctx)?;
                    templates::write_template(
                        &content,
                        &project_dir.join(".dev.vars.example"),
                    )?;
                }
                _ => {}
            }
        }

        state.mark_complete("write_env_examples", None);
        state.save()?;
    }

    // Step 9: Create Cloudflare resources
    if config.create_resources {
        if state.is_step_complete("create_resources", None) {
            ui.skipped("Cloudflare resources");
        } else {
            ui.section("Setting up Cloudflare resources");
            resources::create_resources(ui, config, state, root).await?;
            state.mark_complete("create_resources", None);
            state.save()?;
            ui.newline();
        }
    }

    // Step 10: Bind resource IDs to wrangler.jsonc
    if config.create_resources && !state.created_resources.is_empty() {
        if state.is_step_complete("bind_resources", None) {
            ui.skipped("Resource ID binding");
        } else {
            for project in &config.projects {
                if !project.is_worker() {
                    continue;
                }
                let project_dir = root.join(&project.name);
                let wrangler_path = project_dir.join("wrangler.jsonc");

                if wrangler_path.exists() {
                    let mut content = std::fs::read_to_string(&wrangler_path)?;

                    // Replace D1 database_id placeholder
                    if project.has_resource(&Resource::D1) {
                        if let Some(resource) = state
                            .created_resources
                            .iter()
                            .find(|r| r.resource_type == "d1")
                        {
                            content = content.replace(
                                "\"database_id\": \"\"",
                                &format!("\"database_id\": \"{}\"", resource.id),
                            );
                        }
                    }

                    // Replace KV namespace id placeholder
                    if project.has_resource(&Resource::Kv) {
                        if let Some(resource) = state
                            .created_resources
                            .iter()
                            .find(|r| r.resource_type == "kv")
                        {
                            content = content.replace(
                                "\"id\": \"\"",
                                &format!("\"id\": \"{}\"", resource.id),
                            );
                        }
                    }

                    std::fs::write(&wrangler_path, content)?;
                    ui.success(&format!(
                        "Bound resource IDs to {}/wrangler.jsonc",
                        project.name
                    ));
                }
            }

            state.mark_complete("bind_resources", None);
            state.save()?;
        }
    }

    // Step 11: Run cf-typegen per worker
    if state.is_step_complete("cf_typegen", None) {
        ui.skipped("Type generation");
    } else {
        for project in &config.projects {
            if !project.is_worker() {
                continue;
            }
            let project_dir = root.join(&project.name);
            ui.run_command(
                &format!("Generated types for {}", project.name),
                "npm run cf-typegen",
                &project_dir,
            )
            .await?;
        }

        state.mark_complete("cf_typegen", None);
        state.save()?;
    }

    // Step 12: Run db:generate + db:migrate:local for D1 workers
    if state.is_step_complete("db_setup", None) {
        ui.skipped("Database setup");
    } else {
        for project in &config.projects {
            if !project.has_resource(&Resource::D1) {
                continue;
            }
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

            if config.remote {
                ui.run_command(
                    &format!("Applied remote migrations for {}", project.name),
                    "npm run db:migrate:remote",
                    &project_dir,
                )
                .await?;
            }
        }

        state.mark_complete("db_setup", None);
        state.save()?;
    }

    // Step 13: Write project files
    if state.is_step_complete("write_project_files", None) {
        ui.skipped("Project files");
    } else {
        ui.section("Writing project files");

        let ctx = engine.project_files_context(config, &port_map);

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
        std::fs::create_dir_all(root.join(".github/workflows"))?;
        for project in &config.projects {
            let deploy_ctx = engine.deploy_context(&project.name, &project.name);
            let template = match project.project_type {
                ProjectType::Web => "deploy-web.yml",
                ProjectType::ApiWorker | ProjectType::LightweightWorker => "deploy-worker.yml",
                _ => continue,
            };
            let content = engine.render(template, &deploy_ctx)?;
            templates::write_template(
                &content,
                &root.join(format!(".github/workflows/deploy-{}.yml", project.name)),
            )?;
            ui.success(&format!(".github/workflows/deploy-{}.yml", project.name));
        }

        // .claude/settings.local.json
        let content = engine.render("settings.local.json", &ctx)?;
        templates::write_template(&content, &root.join(".claude/settings.local.json"))?;
        ui.success(".claude/settings.local.json");

        state.mark_complete("write_project_files", None);
        state.save()?;
        ui.newline();
    }

    // Step 14: Git init + initial commit
    if state.is_step_complete("git_init", None) {
        ui.skipped("Git init");
    } else {
        ui.section("Initializing git");

        ui.run_command("Initialized git repository", "git init", root).await?;

        ui.run_command(
            "Initial commit — ready to go",
            "git add -A && git commit -m \"chore: initial project scaffold via launchpad\"",
            root,
        )
        .await?;

        state.mark_complete("git_init", None);
        state.save()?;
        ui.newline();
    }

    // Success — delete state file
    state.delete()?;

    ui.done();

    Ok(())
}
```

- [ ] **Step 2: Add pipeline module to mod.rs**

In `src/commands/launchpad/mod.rs`, add:

```rust
mod pipeline;
```

- [ ] **Step 3: Update mod.rs run() to use the pipeline**

Replace the `run()` function in `src/commands/launchpad/mod.rs`:

```rust
mod clean;
mod config;
mod deps;
mod pipeline;
mod ports;
mod resources;
mod scaffold;
mod state;
mod templates;
mod ui;
mod validation;

use anyhow::{bail, Result};
use std::path::PathBuf;

pub async fn run(config_path: PathBuf, do_clean: bool) -> Result<()> {
    let ui = ui::Ui::new();

    // Load and parse config
    let config_content = std::fs::read_to_string(&config_path)
        .map_err(|e| anyhow::anyhow!("Failed to read config file '{}': {}", config_path.display(), e))?;

    let config: config::LaunchpadConfig = serde_json::from_str(&config_content)
        .map_err(|e| anyhow::anyhow!("Invalid config JSON: {}", e))?;

    // Determine root directory
    let root = if config.root == "." {
        std::env::current_dir()?
    } else {
        std::env::current_dir()?.join(&config.root)
    };

    // Validate
    let errors = validation::validate(&config, &root);
    if !errors.is_empty() {
        let count = errors.len();
        let mut msg = format!(
            "\n  Launchpad 🚀\n\n  ✗ Config validation failed ({} error{}):\n",
            count,
            if count == 1 { "" } else { "s" }
        );
        for (i, error) in errors.iter().enumerate() {
            msg.push_str(&format!("\n  {}. {}\n", i + 1, error));
        }
        bail!("{}", msg);
    }

    let config_hash = state::hash_config(&config_content);

    // Check for existing state (resume or clean)
    let mut launchpad_state = if let Some(existing_state) = state::LaunchpadState::load(&root)? {
        if do_clean {
            ui.header();
            clean::clean_previous_run(&ui, &config, &existing_state, &root).await?;
            state::LaunchpadState::new(config_hash, &root)
        } else if existing_state.config_changed(&config_hash) {
            // Config changed — re-run from first failure point
            let mut new_state = state::LaunchpadState::new(config_hash, &root);
            // Copy completed steps up to first failure
            if let Some(fail_idx) = existing_state.first_failure_index() {
                for step in &existing_state.completed_steps[..fail_idx] {
                    if step.result == state::StepResult::Ok {
                        new_state.completed_steps.push(step.clone());
                    }
                }
            }
            new_state.created_resources = existing_state.created_resources.clone();
            new_state
        } else {
            existing_state
        }
    } else {
        state::LaunchpadState::new(config_hash, &root)
    };

    ui.header();

    // Run the pipeline
    match pipeline::run_pipeline(&config, &config_content, &root, &mut launchpad_state, &ui).await
    {
        Ok(()) => Ok(()),
        Err(e) => {
            launchpad_state.save()?;

            // Print summary of what was completed
            let completed: Vec<_> = launchpad_state
                .completed_steps
                .iter()
                .filter(|s| s.result == state::StepResult::Ok)
                .collect();
            if !completed.is_empty() {
                ui.newline();
                ui.section("Completed before failure:");
                for step in &completed {
                    if let Some(project) = &step.project {
                        ui.success(&format!("{} ({})", step.step, project));
                    } else {
                        ui.success(&step.step);
                    }
                }
            }

            ui.newline();
            ui.failure(&format!(
                "Re-run the same command to resume from where it stopped."
            ));
            ui.newline();

            Err(e)
        }
    }
}
```

- [ ] **Step 4: Build and verify**

Run: `cargo build 2>&1 | tail -5`

Expected: Build succeeds.

- [ ] **Step 5: Commit**

```bash
git add src/commands/launchpad/pipeline.rs src/commands/launchpad/mod.rs
git commit -m "feat(launchpad): add full execution pipeline with resume support"
```

---

## Chunk 7: Integration Testing

### Task 14: End-to-end test with a real config

**Files:**
- No new files — manual testing

- [ ] **Step 1: Build the release binary**

Run: `cargo build --release 2>&1 | tail -5`

Expected: Build succeeds.

- [ ] **Step 2: Create a test config**

Create `/tmp/launchpad-e2e-test.json`:

```json
{
  "name": "testapp",
  "root": "testapp",
  "description": "End-to-end test for launchpad CLI",
  "domain": "testapp.groo.bot",
  "projects": [
    {
      "name": "dashboard",
      "type": "web",
      "auth": "clerk"
    },
    {
      "name": "api",
      "type": "api-worker",
      "auth": "clerk",
      "email": "resend",
      "resources": ["d1", "kv"]
    }
  ],
  "create_resources": false,
  "remote": false
}
```

- [ ] **Step 3: Run the launchpad command**

Run from `/tmp`:

```bash
cd /tmp && cargo run --manifest-path /Users/groo/work/gr/cli/Cargo.toml -- launchpad --config launchpad-e2e-test.json
```

Observe the output:
- Scaffolding spinner and collapse should work
- All config files should be generated
- Git should be initialized

- [ ] **Step 4: Verify generated files**

Check that these files exist and have correct content:
- `/tmp/testapp/dashboard/vite.config.ts` — has port and proxy
- `/tmp/testapp/api/wrangler.jsonc` — has D1 and KV bindings, routes
- `/tmp/testapp/api/src/index.ts` — Hono entry point
- `/tmp/testapp/api/src/config.ts` — has clerk and resend getters
- `/tmp/testapp/dashboard/src/config.ts` — has clerk publishable key getter
- `/tmp/testapp/dashboard/src/lib/api.ts` — axios client
- `/tmp/testapp/CLAUDE.md` — has coding practices
- `/tmp/testapp/README.md` — has project structure
- `/tmp/testapp/TODO.md` — has setup checklist
- `/tmp/testapp/.gitignore` — has node_modules, dist, etc.
- `/tmp/testapp/.github/workflows/deploy-api.yml` — worker workflow
- `/tmp/testapp/.github/workflows/deploy-dashboard.yml` — web workflow
- `/tmp/testapp/.claude/settings.local.json` — has migration deny rules

- [ ] **Step 5: Test resume behavior**

Delete one project and re-run to test resume:

```bash
rm -rf /tmp/testapp/api
cd /tmp && cargo run --manifest-path /Users/groo/work/gr/cli/Cargo.toml -- launchpad --config launchpad-e2e-test.json
```

Expected: Should skip dashboard (already complete) and re-scaffold api.

- [ ] **Step 6: Test clean behavior**

```bash
cd /tmp && cargo run --manifest-path /Users/groo/work/gr/cli/Cargo.toml -- launchpad --config launchpad-e2e-test.json --clean
```

Expected: Should delete testapp/ and start fresh.

- [ ] **Step 7: Test validation errors**

Create a bad config:

```json
{
  "name": "bad-test",
  "root": ".",
  "description": "Bad config test",
  "projects": [
    {
      "name": "web",
      "type": "web",
      "resources": ["d1"]
    }
  ],
  "create_resources": false,
  "remote": false
}
```

Run: `cargo run -- launchpad --config /tmp/bad-config.json`

Expected: Clear validation error about web project having resources.

- [ ] **Step 8: Clean up test artifacts**

```bash
rm -rf /tmp/testapp /tmp/launchpad-e2e-test.json /tmp/bad-config.json
```

- [ ] **Step 9: Final commit — update plan status**

No code changes — just verify everything works. If any fixes were needed during testing, they should have been committed in their respective steps.

---

## Chunk 8: Verify npm create cloudflare flags

### Task 15: Verify non-interactive cloudflare worker scaffolding

**Files:**
- Possibly modify: `src/commands/launchpad/scaffold.rs`

- [ ] **Step 1: Test npm create cloudflare flags**

Run in a temp directory:

```bash
cd /tmp && npm create cloudflare@latest test-worker -- --type hello-world --no-git --no-deploy 2>&1
```

Observe:
- Does it complete without interactive prompts?
- If it still prompts, check `npm create cloudflare -- --help` for available flags
- Common alternatives: `--no-open`, `--ts`, `--existing-script`

- [ ] **Step 2: Update scaffold.rs if flags need adjustment**

If the flags from step 1 don't work non-interactively, update the command in `src/commands/launchpad/scaffold.rs` with the correct flags.

- [ ] **Step 3: Clean up and commit if changed**

```bash
rm -rf /tmp/test-worker
```

If `scaffold.rs` was modified:

```bash
git add src/commands/launchpad/scaffold.rs
git commit -m "fix(launchpad): correct non-interactive cloudflare worker scaffold flags"
```
