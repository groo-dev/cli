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

    let config_content = std::fs::read_to_string(&config_path)
        .map_err(|e| anyhow::anyhow!("Failed to read config file '{}': {}", config_path.display(), e))?;

    let config: config::LaunchpadConfig = serde_json::from_str(&config_content)
        .map_err(|e| anyhow::anyhow!("Invalid config JSON: {}", e))?;

    let root = if config.root == "." {
        std::env::current_dir()?
    } else {
        std::env::current_dir()?.join(&config.root)
    };

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

    let mut launchpad_state = if let Some(existing_state) = state::LaunchpadState::load(&root)? {
        if do_clean {
            ui.header();
            clean::clean_previous_run(&ui, &config, &existing_state, &root).await?;
            state::LaunchpadState::new(config_hash, &root)
        } else if existing_state.config_changed(&config_hash) {
            let mut new_state = state::LaunchpadState::new(config_hash, &root);
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

    match pipeline::run_pipeline(&config, &config_content, &root, &mut launchpad_state, &ui).await {
        Ok(()) => Ok(()),
        Err(e) => {
            launchpad_state.save()?;

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
            ui.failure("Re-run the same command to resume from where it stopped.");
            ui.newline();

            Err(e)
        }
    }
}
