use std::path::Path;
use std::process::Command;

use crate::SieveError;

/// Get list of changed files from git diff or explicit file list
pub fn get_changed_files(
    repo: &Path,
    base: Option<&str>,
    explicit_files: &[String],
) -> Result<Vec<String>, SieveError> {
    // If explicit files provided, use those
    if !explicit_files.is_empty() {
        return Ok(explicit_files.to_vec());
    }

    // Try staged changes first
    let staged = git_diff_names(repo, &["--cached", "--name-only"])?;
    if !staged.is_empty() {
        return Ok(staged);
    }

    // Try unstaged changes against base or HEAD
    let base = base.unwrap_or("HEAD");
    let unstaged = git_diff_names(repo, &["--name-only", base])?;
    if !unstaged.is_empty() {
        return Ok(unstaged);
    }

    // Try untracked files
    let untracked = git_untracked(repo)?;
    Ok(untracked)
}

fn git_diff_names(repo: &Path, args: &[&str]) -> Result<Vec<String>, SieveError> {
    let mut cmd_args = vec!["diff"];
    cmd_args.extend(args);

    let output = Command::new("git")
        .args(&cmd_args)
        .current_dir(repo)
        .output()
        .map_err(|e| SieveError::Io(e))?;

    if !output.status.success() {
        return Ok(Vec::new());
    }

    let text = String::from_utf8_lossy(&output.stdout);
    Ok(text.lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect())
}

fn git_untracked(repo: &Path) -> Result<Vec<String>, SieveError> {
    let output = Command::new("git")
        .args(["ls-files", "--others", "--exclude-standard"])
        .current_dir(repo)
        .output()
        .map_err(|e| SieveError::Io(e))?;

    if !output.status.success() {
        return Ok(Vec::new());
    }

    let text = String::from_utf8_lossy(&output.stdout);
    Ok(text.lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect())
}
