use anyhow::Result;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::Service;

/// Context for determining which git diff to use
#[derive(Debug, Clone, Copy)]
pub enum ChangeContext {
    /// Pre-commit hook: staged files
    PreCommit,
    /// Pre-push hook: local commits not yet pushed
    PrePush,
    /// Manual run: uncommitted changes
    Manual,
}

impl ChangeContext {
    /// Detect the current context based on environment variables set by git hooks
    pub fn detect() -> Self {
        // GIT_INDEX_FILE is set during pre-commit hooks
        if std::env::var("GIT_INDEX_FILE").is_ok() {
            return ChangeContext::PreCommit;
        }

        // GIT_PUSH_OPTION_COUNT is set during pre-push hooks
        if std::env::var("GIT_PUSH_OPTION_COUNT").is_ok() {
            return ChangeContext::PrePush;
        }

        ChangeContext::Manual
    }

    /// Get the appropriate git diff arguments for this context
    fn git_diff_args(&self) -> Vec<&str> {
        match self {
            ChangeContext::PreCommit => vec!["diff", "--cached", "--name-only"],
            ChangeContext::PrePush => vec!["diff", "@{push}..HEAD", "--name-only"],
            ChangeContext::Manual => vec!["diff", "HEAD", "--name-only"],
        }
    }
}

/// Get list of changed files based on the current context
pub fn get_changed_files(root: &Path) -> Result<Vec<PathBuf>> {
    let context = ChangeContext::detect();
    get_changed_files_for_context(root, context)
}

/// Get list of changed files for a specific context
pub fn get_changed_files_for_context(root: &Path, context: ChangeContext) -> Result<Vec<PathBuf>> {
    let output = Command::new("git")
        .args(context.git_diff_args())
        .current_dir(root)
        .output()?;

    if !output.status.success() {
        // If git diff fails (e.g., no upstream for @{push}), try fallback
        if matches!(context, ChangeContext::PrePush) {
            // Fallback: compare with origin/main or origin/master
            let fallback = Command::new("git")
                .args(["diff", "origin/main...HEAD", "--name-only"])
                .current_dir(root)
                .output();

            if let Ok(fallback_output) = fallback {
                if fallback_output.status.success() {
                    return Ok(parse_file_list(root, &fallback_output.stdout));
                }
            }

            // Try origin/master as last resort
            let fallback = Command::new("git")
                .args(["diff", "origin/master...HEAD", "--name-only"])
                .current_dir(root)
                .output();

            if let Ok(fallback_output) = fallback {
                if fallback_output.status.success() {
                    return Ok(parse_file_list(root, &fallback_output.stdout));
                }
            }
        }

        // Return empty list if we can't determine changes
        return Ok(Vec::new());
    }

    Ok(parse_file_list(root, &output.stdout))
}

fn parse_file_list(root: &Path, stdout: &[u8]) -> Vec<PathBuf> {
    String::from_utf8_lossy(stdout)
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| root.join(l))
        .collect()
}

/// Filter services to only those with changed files
pub fn filter_services_with_changes<'a>(
    services: &'a [Service],
    changed_files: &[PathBuf],
) -> Vec<&'a Service> {
    services
        .iter()
        .filter(|service| {
            changed_files
                .iter()
                .any(|file| file.starts_with(&service.path))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_detection_default() {
        // Without special env vars, should be Manual
        std::env::remove_var("GIT_INDEX_FILE");
        std::env::remove_var("GIT_PUSH_OPTION_COUNT");
        assert!(matches!(ChangeContext::detect(), ChangeContext::Manual));
    }
}
