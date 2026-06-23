use std::collections::HashMap;
use std::path::Path;
use serde::{Deserialize, Serialize};

use crate::conventions;
use crate::detect::{self, DetectedProject};
use crate::history;
use crate::imports;
use crate::SieveError;

/// Sentinel's risk level for a file (loaded from .agent-sentinel/matrix.json)
#[derive(Debug, Deserialize)]
struct SentinelMatrix {
    files: Vec<SentinelFileRisk>,
}

/// Minimal atlas graph structs — only what we need to build rdeps.
/// Atlas serializes `rdeps` with `#[serde(skip)]`, so we rebuild from forward deps.
#[derive(Debug, Deserialize)]
struct AtlasGraph {
    nodes: HashMap<String, AtlasNode>,
}

#[derive(Debug, Deserialize)]
struct AtlasNode {
    deps: Vec<String>,
}

/// Max atlas-sourced signals to prevent noise on hub files.
const ATLAS_SIGNAL_CAP: usize = 10;

#[derive(Debug, Deserialize)]
struct SentinelFileRisk {
    path: String,
    risk_score: u32,
    level: String,
    bugfix_commits: usize,
    related_tests: Vec<SentinelRelatedTest>,
    reasons: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct SentinelRelatedTest {
    path: String,
    #[allow(dead_code)]
    cochanges: usize,
}

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

    // Signal 5: Sentinel fragility matrix (high confidence)
    if let Some(matrix) = load_sentinel_matrix(repo) {
        for file in changed_files {
            if is_test_file(file, primary_kind) {
                continue;
            }
            if let Some(risk) = matrix.files.iter().find(|f| f.path == *file) {
                if risk.level == "high" || risk.level == "medium" {
                    // Add signals for sentinel's related tests
                    for related in &risk.related_tests {
                        if signals.iter().any(|s| {
                            s.changed_file == *file && s.test_file.as_deref() == Some(related.path.as_str())
                        }) {
                            continue; // Already found by other signals
                        }
                        signals.push(Signal {
                            kind: "sentinel_risk".into(),
                            changed_file: file.clone(),
                            test_file: Some(related.path.clone()),
                            command: test_command_for_file(&related.path, primary_kind),
                            confidence: if risk.level == "high" { "certain" } else { "high" }.into(),
                            reason: format!(
                                "Sentinel risk {} (score {}, {} bugfix commits): {}",
                                risk.level, risk.risk_score, risk.bugfix_commits,
                                risk.reasons.first().map(|s| s.as_str()).unwrap_or("fragile file")
                            ),
                        });
                    }
                    // If sentinel knows the file is risky but has no related tests, promote to full suite
                    if risk.related_tests.is_empty() && !signals.iter().any(|s| s.changed_file == *file) {
                        signals.push(Signal {
                            kind: "sentinel_risk".into(),
                            changed_file: file.clone(),
                            test_file: None,
                            command: projects.first().and_then(|p| detect::test_command_for(p)),
                            confidence: if risk.level == "high" { "certain" } else { "high" }.into(),
                            reason: format!(
                                "Sentinel risk {} (score {}) — full test suite recommended",
                                risk.level, risk.risk_score
                            ),
                        });
                    }
                }
            }
        }
    }

    // Signal 6: Atlas reverse dependencies (high confidence)
    if let Some(rdeps_map) = load_atlas_rdeps(repo) {
        let mut atlas_signal_count = 0;
        for file in changed_files {
            if atlas_signal_count >= ATLAS_SIGNAL_CAP {
                break;
            }
            if is_test_file(file, primary_kind) {
                continue;
            }
            let normalized = normalize_path(file);
            if let Some(dependents) = rdeps_map.get(&normalized) {
                for dep in dependents {
                    if atlas_signal_count >= ATLAS_SIGNAL_CAP {
                        break;
                    }
                    if !is_test_file(dep, primary_kind) {
                        continue; // Only surface test files
                    }
                    // Skip if already found by other signals
                    if signals.iter().any(|s| {
                        s.changed_file == *file && s.test_file.as_deref() == Some(dep.as_str())
                    }) {
                        continue;
                    }
                    signals.push(Signal {
                        kind: "atlas_rdep".into(),
                        changed_file: file.clone(),
                        test_file: Some(dep.clone()),
                        command: test_command_for_file(dep, primary_kind),
                        confidence: "high".into(),
                        reason: format!("Atlas: test file depends on changed file"),
                    });
                    atlas_signal_count += 1;
                }
            }
        }
    }

    // Signal 7: Fallback project-level command (low confidence)
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

/// Normalize a file path for comparison: strip leading `./`, use forward slashes.
fn normalize_path(path: &str) -> String {
    let p = path.strip_prefix("./").unwrap_or(path);
    p.replace('\\', "/")
}

/// Load atlas graph and build reverse dependency map.
/// Returns None silently if graph doesn't exist or can't be parsed.
fn load_atlas_rdeps(repo: &Path) -> Option<HashMap<String, Vec<String>>> {
    let path = repo.join(".agent-atlas").join("graph.json");
    let content = std::fs::read_to_string(path).ok()?;
    let graph: AtlasGraph = serde_json::from_str(&content).ok()?;

    let mut rdeps: HashMap<String, Vec<String>> = HashMap::new();
    for (file_path, node) in &graph.nodes {
        let normalized_source = normalize_path(file_path);
        for dep in &node.deps {
            let normalized_dep = normalize_path(dep);
            // Skip self-edges
            if normalized_dep == normalized_source {
                continue;
            }
            rdeps
                .entry(normalized_dep)
                .or_default()
                .push(normalized_source.clone());
        }
    }

    Some(rdeps)
}

fn load_sentinel_matrix(repo: &Path) -> Option<SentinelMatrix> {
    let path = repo.join(".agent-sentinel").join("matrix.json");
    let content = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
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
