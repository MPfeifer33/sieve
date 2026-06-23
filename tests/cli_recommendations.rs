//! Integration tests for collapsed recommendation output.

use std::fs;
use std::path::Path;
use std::process::{Command, Output};
use tempfile::TempDir;

fn sieve(dir: &Path) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_sieve"));
    cmd.arg("--repo").arg(dir);
    cmd
}

fn assert_success(output: &Output, label: &str) {
    assert!(
        output.status.success(),
        "{label} failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn json_output(output: Output, label: &str) -> serde_json::Value {
    assert_success(&output, label);
    serde_json::from_slice(&output.stdout).unwrap_or_else(|err| {
        panic!(
            "{label} returned invalid json: {err}\nstdout:\n{}",
            String::from_utf8_lossy(&output.stdout)
        )
    })
}

fn rust_project(dir: &Path) {
    fs::write(
        dir.join("Cargo.toml"),
        r#"[package]
name = "sample"
version = "0.1.0"
edition = "2021"
"#,
    )
    .unwrap();
    fs::create_dir_all(dir.join("src")).unwrap();
    fs::create_dir_all(dir.join("tests")).unwrap();
}

#[test]
fn json_includes_ranked_recommendations_for_convention_match() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();
    rust_project(dir);
    fs::write(dir.join("src/claims.rs"), "pub fn claim() {}\n").unwrap();
    fs::write(
        dir.join("tests/cli_claims.rs"),
        "#[test] fn claim_cli() {}\n",
    )
    .unwrap();

    let json = json_output(
        sieve(dir)
            .args(["--format", "json", "analyze", "--file", "src/claims.rs"])
            .output()
            .unwrap(),
        "analyze json",
    );

    assert_eq!(json["ok"], true);
    assert_eq!(json["changed_files"][0], "src/claims.rs");
    assert_eq!(
        json["recommendations"][0]["command"],
        "cargo test --test cli_claims"
    );
    assert_eq!(json["recommendations"][0]["confidence"], "high");
    assert_eq!(
        json["recommendations"][0]["test_files"][0],
        "tests/cli_claims.rs"
    );
    assert!(json["recommendations"][0]["reasons"][0]
        .as_str()
        .unwrap()
        .contains("Convention"));
    assert!(json["coverage_gaps"].as_array().unwrap().is_empty());
    assert!(!json["signals"].as_array().unwrap().is_empty());
}

#[test]
fn fallback_recommendation_reports_coverage_gap() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();
    rust_project(dir);
    fs::write(dir.join("src/orphan.rs"), "pub fn orphan() {}\n").unwrap();

    let json = json_output(
        sieve(dir)
            .args(["--format", "json", "analyze", "--file", "src/orphan.rs"])
            .output()
            .unwrap(),
        "analyze json",
    );

    assert_eq!(json["recommendations"][0]["command"], "cargo test");
    assert_eq!(json["recommendations"][0]["confidence"], "low");
    assert_eq!(json["coverage_gaps"][0]["changed_file"], "src/orphan.rs");
    assert!(json["coverage_gaps"][0]["reason"]
        .as_str()
        .unwrap()
        .contains("No targeted test"));
}

#[test]
fn atlas_rdep_surfaces_test_from_dependency_graph() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();
    rust_project(dir);
    // Source file — named so convention and import matching won't find the test
    fs::write(dir.join("src/core.rs"), "pub fn run() {}\n").unwrap();
    // A test file with a non-matching name that only atlas knows depends on core.rs
    fs::write(
        dir.join("tests/smoke.rs"),
        "#[test] fn it_runs() {}\n",
    )
    .unwrap();

    // Write an atlas graph where tests/smoke.rs depends on src/core.rs
    let atlas_dir = dir.join(".agent-atlas");
    fs::create_dir_all(&atlas_dir).unwrap();
    fs::write(
        atlas_dir.join("graph.json"),
        r#"{
            "nodes": {
                "src/core.rs": {
                    "path": "src/core.rs",
                    "language": "rust",
                    "imports": [],
                    "deps": [],
                    "exports": ["run"],
                    "lines": 1
                },
                "tests/smoke.rs": {
                    "path": "tests/smoke.rs",
                    "language": "rust",
                    "imports": [],
                    "deps": ["src/core.rs"],
                    "exports": [],
                    "lines": 1
                }
            }
        }"#,
    )
    .unwrap();

    let json = json_output(
        sieve(dir)
            .args(["--format", "json", "analyze", "--file", "src/core.rs"])
            .output()
            .unwrap(),
        "atlas rdep analyze",
    );

    assert_eq!(json["ok"], true);
    // Should have an atlas_rdep signal (not found by convention or import matching)
    let signals = json["signals"].as_array().unwrap();
    let atlas_signal = signals
        .iter()
        .find(|s| s["kind"] == "atlas_rdep");
    assert!(
        atlas_signal.is_some(),
        "Expected atlas_rdep signal, got: {signals:?}"
    );
    let sig = atlas_signal.unwrap();
    assert_eq!(sig["changed_file"], "src/core.rs");
    assert_eq!(sig["test_file"], "tests/smoke.rs");
    assert_eq!(sig["confidence"], "high");
}

#[test]
fn text_output_highlights_recommendations_and_full_validation() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();
    rust_project(dir);
    fs::write(dir.join("src/claims.rs"), "pub fn claim() {}\n").unwrap();
    fs::write(
        dir.join("tests/cli_claims.rs"),
        "#[test] fn claim_cli() {}\n",
    )
    .unwrap();

    let output = sieve(dir)
        .args(["analyze", "--file", "src/claims.rs"])
        .output()
        .unwrap();
    assert_success(&output, "analyze text");
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(stdout.contains("Recommendations"));
    assert!(stdout.contains("cargo test --test cli_claims"));
    assert!(stdout.contains("Full validation"));
}
