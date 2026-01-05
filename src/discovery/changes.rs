use anyhow::Result;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::Service;

/// Get list of files changed in unpushed commits
/// Uses `git diff @{u}..HEAD` to compare with upstream tracking branch
pub fn get_changed_files(root: &Path) -> Result<Vec<PathBuf>> {
    let output = Command::new("git")
        .args(["diff", "@{u}..HEAD", "--name-only"])
        .current_dir(root)
        .output()?;

    // If no upstream or other error, return empty list
    if !output.status.success() {
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
