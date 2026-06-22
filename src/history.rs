use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

/// For each changed file, find test files that have historically been modified
/// in the same commits. Returns a map of changed_file -> [(test_file, co_change_count)]
pub fn find_co_changed_tests(
    repo: &Path,
    changed_files: &[String],
    test_files: &[String],
    max_commits: usize,
) -> HashMap<String, Vec<(String, usize)>> {
    let mut results: HashMap<String, Vec<(String, usize)>> = HashMap::new();

    for changed_file in changed_files {
        // Get commits that touched this file
        let commits = get_commits_for_file(repo, changed_file, max_commits);

        // For each commit, find which test files were also changed
        let mut co_changes: HashMap<String, usize> = HashMap::new();
        for commit in &commits {
            let files_in_commit = get_files_in_commit(repo, commit);
            for test_file in test_files {
                if files_in_commit.contains(test_file) {
                    *co_changes.entry(test_file.clone()).or_insert(0) += 1;
                }
            }
        }

        // Sort by frequency
        let mut sorted: Vec<(String, usize)> = co_changes.into_iter().collect();
        sorted.sort_by(|a, b| b.1.cmp(&a.1));
        results.insert(changed_file.clone(), sorted);
    }

    results
}

fn get_commits_for_file(repo: &Path, file: &str, max: usize) -> Vec<String> {
    let output = Command::new("git")
        .args(["log", "--format=%H", &format!("-{max}"), "--", file])
        .current_dir(repo)
        .output();

    match output {
        Ok(o) if o.status.success() => {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .map(|l| l.trim().to_string())
                .filter(|l| !l.is_empty())
                .collect()
        }
        _ => Vec::new(),
    }
}

fn get_files_in_commit(repo: &Path, commit: &str) -> Vec<String> {
    let output = Command::new("git")
        .args(["diff-tree", "--no-commit-id", "-r", "--name-only", commit])
        .current_dir(repo)
        .output();

    match output {
        Ok(o) if o.status.success() => {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .map(|l| l.trim().to_string())
                .filter(|l| !l.is_empty())
                .collect()
        }
        _ => Vec::new(),
    }
}
