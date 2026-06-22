use std::path::Path;

/// Find test files that correspond to changed source files by naming convention.
/// Returns Vec<(changed_file, test_file, convention_name)>
pub fn find_convention_matches(
    repo: &Path,
    changed_files: &[String],
    project_kind: &str,
) -> Vec<(String, String, String)> {
    let mut matches = Vec::new();

    for file in changed_files {
        let candidates = generate_test_candidates(file, project_kind);
        for (candidate, convention) in candidates {
            if repo.join(&candidate).exists() {
                matches.push((file.clone(), candidate, convention));
            }
        }
    }

    matches
}

/// Discover all test files in a project
pub fn discover_test_files(repo: &Path, project_kind: &str) -> Vec<String> {
    match project_kind {
        "rust" => discover_rust_tests(repo),
        "node" => discover_node_tests(repo),
        "python" => discover_python_tests(repo),
        "go" => discover_go_tests(repo),
        _ => Vec::new(),
    }
}

fn generate_test_candidates(file: &str, kind: &str) -> Vec<(String, String)> {
    let mut candidates = Vec::new();
    let path = Path::new(file);
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    let parent = path.parent().map(|p| p.to_string_lossy().to_string()).unwrap_or_default();

    match kind {
        "rust" => {
            // src/foo.rs -> tests/foo.rs
            if file.starts_with("src/") {
                let test_path = file.replacen("src/", "tests/", 1);
                candidates.push((test_path.clone(), "src→tests mirror".into()));

                // src/foo.rs -> tests/foo_test.rs (less common but exists)
                let test_path2 = format!("tests/{stem}_test.rs");
                candidates.push((test_path2, "src→tests_test".into()));
            }

            // src/foo.rs -> tests/cli_foo.rs (integration test pattern)
            if file.starts_with("src/") {
                let test_path = format!("tests/cli_{stem}.rs");
                candidates.push((test_path, "src→cli_test".into()));
            }
        }
        "node" => {
            // src/foo.js -> src/foo.test.js
            candidates.push((format!("{}/{stem}.test.{ext}", parent), "co-located test".into()));
            // src/foo.js -> __tests__/foo.test.js
            candidates.push((format!("__tests__/{stem}.test.{ext}"), "__tests__ dir".into()));
            // src/foo.js -> test/foo.test.js
            candidates.push((format!("test/{stem}.test.{ext}"), "test dir".into()));
            // src/foo.ts -> src/foo.spec.ts
            candidates.push((format!("{}/{stem}.spec.{ext}", parent), "spec file".into()));
        }
        "python" => {
            // module/foo.py -> tests/test_foo.py
            candidates.push((format!("tests/test_{stem}.py"), "tests/test_".into()));
            // module/foo.py -> test_foo.py (same dir)
            candidates.push((format!("{}/test_{stem}.py", parent), "co-located test_".into()));
        }
        "go" => {
            // foo.go -> foo_test.go (same dir)
            if !file.ends_with("_test.go") {
                candidates.push((format!("{}/{stem}_test.go", parent), "go _test.go".into()));
            }
        }
        _ => {}
    }

    candidates
}

fn discover_rust_tests(repo: &Path) -> Vec<String> {
    let mut tests = Vec::new();

    // tests/ directory
    let tests_dir = repo.join("tests");
    if tests_dir.exists() {
        if let Ok(entries) = std::fs::read_dir(&tests_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map_or(false, |e| e == "rs") {
                    if let Ok(rel) = path.strip_prefix(repo) {
                        tests.push(rel.to_string_lossy().to_string());
                    }
                }
            }
        }
    }

    // Also find #[cfg(test)] modules by looking for files containing it
    // (simplified: just check src/ files for test modules)
    tests
}

fn discover_node_tests(repo: &Path) -> Vec<String> {
    let mut tests = Vec::new();
    let patterns = ["__tests__", "test", "tests"];

    for dir_name in patterns {
        let dir = repo.join(dir_name);
        if dir.exists() {
            collect_test_files_recursive(&dir, repo, &mut tests, &["test.js", "test.ts", "spec.js", "spec.ts", "test.jsx", "test.tsx"]);
        }
    }

    // Also find co-located tests in src/
    let src_dir = repo.join("src");
    if src_dir.exists() {
        collect_test_files_recursive(&src_dir, repo, &mut tests, &["test.js", "test.ts", "spec.js", "spec.ts", "test.jsx", "test.tsx"]);
    }

    tests
}

fn discover_python_tests(repo: &Path) -> Vec<String> {
    let mut tests = Vec::new();
    let tests_dir = repo.join("tests");
    if tests_dir.exists() {
        collect_test_files_recursive(&tests_dir, repo, &mut tests, &["test_", "_test.py"]);
    }

    // Also check root for test_ files
    if let Ok(entries) = std::fs::read_dir(repo) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with("test_") && name.ends_with(".py") {
                tests.push(name);
            }
        }
    }

    tests
}

fn discover_go_tests(repo: &Path) -> Vec<String> {
    let mut tests = Vec::new();
    collect_go_tests_recursive(repo, repo, &mut tests);
    tests
}

fn collect_test_files_recursive(dir: &Path, repo: &Path, tests: &mut Vec<String>, patterns: &[&str]) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if path.file_name().map_or(false, |n| n == "node_modules" || n == ".git" || n == "target") {
                    continue;
                }
                collect_test_files_recursive(&path, repo, tests, patterns);
            } else {
                let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
                if patterns.iter().any(|p| name.contains(p)) {
                    if let Ok(rel) = path.strip_prefix(repo) {
                        tests.push(rel.to_string_lossy().to_string());
                    }
                }
            }
        }
    }
}

fn collect_go_tests_recursive(dir: &Path, repo: &Path, tests: &mut Vec<String>) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if path.file_name().map_or(false, |n| n == ".git" || n == "vendor") {
                    continue;
                }
                collect_go_tests_recursive(&path, repo, tests);
            } else if path.to_string_lossy().ends_with("_test.go") {
                if let Ok(rel) = path.strip_prefix(repo) {
                    tests.push(rel.to_string_lossy().to_string());
                }
            }
        }
    }
}
