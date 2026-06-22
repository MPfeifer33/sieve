use std::path::Path;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectedProject {
    pub kind: String,
    pub root: String,
}

pub fn detect_projects(repo: &Path) -> Vec<DetectedProject> {
    let mut projects = Vec::new();

    if repo.join("Cargo.toml").exists() {
        projects.push(DetectedProject { kind: "rust".into(), root: ".".into() });
    }
    if repo.join("package.json").exists() {
        projects.push(DetectedProject { kind: "node".into(), root: ".".into() });
    }
    if repo.join("pyproject.toml").exists() || repo.join("setup.py").exists() || repo.join("requirements.txt").exists() {
        projects.push(DetectedProject { kind: "python".into(), root: ".".into() });
    }
    if repo.join("go.mod").exists() {
        projects.push(DetectedProject { kind: "go".into(), root: ".".into() });
    }
    if repo.join("src-tauri").exists() {
        projects.push(DetectedProject { kind: "tauri".into(), root: "src-tauri".into() });
    }

    projects
}

pub fn test_command_for(project: &DetectedProject) -> Option<String> {
    match project.kind.as_str() {
        "rust" | "tauri" => Some("cargo test".into()),
        "node" => Some("npm test".into()),
        "python" => Some("python -m pytest".into()),
        "go" => Some("go test ./...".into()),
        _ => None,
    }
}
