use std::path::Path;
use serde::{Deserialize, Serialize};

use crate::conventions;
use crate::detect::{self, DetectedProject};
use crate::history;
use crate::imports;
use crate::SieveError;

#[derive(Debug, Serialize, Deserialize)]
pub struct AnalysisResult {
    pub changed_files: Vec<String>,
    pub signals: Vec<Signal>,
    pub full_validation_command: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Signal {
    pub kind: String,
    pub changed_file: String,
    pub test_file: Option<String>,
    pub command: Option<String>,
    pub confidence: String,
    pub reason: String,
}

pub fn run_analysis(
    repo: &Path,
    changed_files: &[String],
    projects: &[DetectedProject],
) -> Result<AnalysisResult, SieveError> {
    let mut signals = Vec::new();

    // Determine primary project kind
    let primary_kind = projects.first().map(|p| p.kind.as_str()).unwrap_or("unknown");

    // Discover existing test files
    let test_files = conventions::discover_test_files(repo, primary_kind);

    // Signal 1: Direct test file changes (certain confidence)
    for file in changed_files {
        if is_test_file(file, primary_kind) {
            signals.push(Signal {
                kind: "direct_test_change".into(),
                changed_file: file.clone(),
                test_file: Some(file.clone()),
                command: test_command_for_file(file, primary_kind),
                confidence: "certain".into(),
                reason: format!("{file} is itself a test file"),
            });
        }
    }

    // Signal 2: Convention-based mapping (high confidence)
    let convention_matches = conventions::find_convention_matches(repo, changed_files, primary_kind);
    for (changed, test, convention) in &convention_matches {
        signals.push(Signal {
            kind: "convention_match".into(),
            changed_file: changed.clone(),
            test_file: Some(test.clone()),
            command: test_command_for_file(test, primary_kind),
            confidence: "high".into(),
            reason: format!("Convention: {convention}"),
        });
    }

    // Signal 3: Import analysis (high confidence)
    for file in changed_files {
        if is_test_file(file, primary_kind) {
            continue; // Already handled
        }
        let module_name = extract_module_name(file, primary_kind);
        if let Some(ref module) = module_name {
            let importing_tests = imports::find_test_files_importing(repo, module, &test_files);
            for test_file in importing_tests {
                // Skip if already found by convention
                if convention_matches.iter().any(|(_, t, _)| t == &test_file) {
                    continue;
                }
                signals.push(Signal {
                    kind: "import_match".into(),
                    changed_file: file.clone(),
                    test_file: Some(test_file.clone()),
                    command: test_command_for_file(&test_file, primary_kind),
                    confidence: "high".into(),
                    reason: format!("Test imports module '{module}'"),
                });
            }
        }
    }

    // Signal 4: Git history correlation (medium confidence)
    let source_files: Vec<String> = changed_files.iter()
        .filter(|f| !is_test_file(f, primary_kind))
        .cloned()
        .collect();

    if !source_files.is_empty() && !test_files.is_empty() {
        let co_changes = history::find_co_changed_tests(repo, &source_files, &test_files, 50);
        for (changed_file, correlated_tests) in &co_changes {
            for (test_file, count) in correlated_tests {
                if *count < 2 {
                    continue; // Require at least 2 co-changes for signal
                }
                // Skip if already found by other signals
                if signals.iter().any(|s| {
                    s.changed_file == *changed_file && s.test_file.as_deref() == Some(test_file.as_str())
                }) {
                    continue;
                }
                signals.push(Signal {
                    kind: "history_correlation".into(),
                    changed_file: changed_file.clone(),
                    test_file: Some(test_file.clone()),
                    command: test_command_for_file(test_file, primary_kind),
                    confidence: "medium".into(),
                    reason: format!("Co-changed in {count} commits"),
                });
            }
        }
    }

    // Signal 5: Fallback project-level command (low confidence)
    if signals.is_empty() {
        for project in projects {
            if let Some(cmd) = detect::test_command_for(project) {
                signals.push(Signal {
                    kind: "fallback".into(),
                    changed_file: changed_files.first().cloned().unwrap_or_default(),
                    test_file: None,
                    command: Some(cmd.clone()),
                    confidence: "low".into(),
                    reason: format!("No specific test found; fallback to project-level: {cmd}"),
                });
            }
        }
    }

    // Full validation command
    let full_command = projects.first()
        .and_then(|p| detect::test_command_for(p));

    Ok(AnalysisResult {
        changed_files: changed_files.to_vec(),
        signals,
        full_validation_command: full_command,
    })
}

fn is_test_file(file: &str, kind: &str) -> bool {
    match kind {
        "rust" => file.starts_with("tests/") || file.contains("/tests/"),
        "node" => file.contains(".test.") || file.contains(".spec.") || file.contains("__tests__"),
        "python" => file.contains("test_") || file.contains("_test.py"),
        "go" => file.ends_with("_test.go"),
        _ => false,
    }
}

fn extract_module_name(file: &str, kind: &str) -> Option<String> {
    let path = Path::new(file);
    let stem = path.file_stem()?.to_str()?;

    match kind {
        "rust" => {
            // src/claims.rs -> "claims"
            // src/db.rs -> "db"
            Some(stem.to_string())
        }
        "node" => {
            // src/utils.ts -> "utils" or "./utils"
            Some(stem.to_string())
        }
        "python" => {
            // package/module.py -> "module"
            Some(stem.to_string())
        }
        "go" => Some(stem.to_string()),
        _ => None,
    }
}

fn test_command_for_file(file: &str, kind: &str) -> Option<String> {
    match kind {
        "rust" => {
            if file.starts_with("tests/") {
                let stem = Path::new(file).file_stem()?.to_str()?;
                Some(format!("cargo test --test {stem}"))
            } else {
                Some("cargo test".into())
            }
        }
        "node" => Some(format!("npx jest {file}")),
        "python" => Some(format!("python -m pytest {file}")),
        "go" => {
            let dir = Path::new(file).parent()?.to_str()?;
            if dir.is_empty() {
                Some("go test .".into())
            } else {
                Some(format!("go test ./{dir}"))
            }
        }
        _ => None,
    }
}
