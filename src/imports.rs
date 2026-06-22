use std::path::Path;
use regex::Regex;

/// Extract module names that a file imports/uses
pub fn extract_imports(repo: &Path, file_path: &str) -> Vec<String> {
    let full_path = repo.join(file_path);
    let content = match std::fs::read_to_string(&full_path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };

    let ext = Path::new(file_path).extension().and_then(|e| e.to_str()).unwrap_or("");
    match ext {
        "rs" => extract_rust_imports(&content),
        "js" | "ts" | "jsx" | "tsx" | "mjs" => extract_js_imports(&content),
        "py" => extract_python_imports(&content),
        "go" => extract_go_imports(&content),
        _ => Vec::new(),
    }
}

/// Find test files that import/use a given module name
pub fn find_test_files_importing(repo: &Path, module_name: &str, test_files: &[String]) -> Vec<String> {
    test_files.iter()
        .filter(|test_file| {
            let imports = extract_imports(repo, test_file);
            imports.iter().any(|imp| imp.contains(module_name))
        })
        .cloned()
        .collect()
}

fn extract_rust_imports(content: &str) -> Vec<String> {
    let re = Regex::new(r"(?m)^\s*use\s+((?:crate|super|self|[a-zA-Z_][a-zA-Z0-9_]*)(?:::[a-zA-Z_][a-zA-Z0-9_]*)*)").unwrap();
    let mut imports = Vec::new();

    for cap in re.captures_iter(content) {
        if let Some(m) = cap.get(1) {
            imports.push(m.as_str().to_string());
        }
    }

    // Also check mod declarations
    let mod_re = Regex::new(r"(?m)^\s*mod\s+([a-zA-Z_][a-zA-Z0-9_]*)").unwrap();
    for cap in mod_re.captures_iter(content) {
        if let Some(m) = cap.get(1) {
            imports.push(format!("mod::{}", m.as_str()));
        }
    }

    imports
}

fn extract_js_imports(content: &str) -> Vec<String> {
    let mut imports = Vec::new();

    // import ... from '...'
    let import_re = Regex::new(r#"(?m)import\s+.*?\s+from\s+['"]([^'"]+)['"]"#).unwrap();
    for cap in import_re.captures_iter(content) {
        if let Some(m) = cap.get(1) {
            imports.push(m.as_str().to_string());
        }
    }

    // require('...')
    let require_re = Regex::new(r#"require\s*\(\s*['"]([^'"]+)['"]\s*\)"#).unwrap();
    for cap in require_re.captures_iter(content) {
        if let Some(m) = cap.get(1) {
            imports.push(m.as_str().to_string());
        }
    }

    imports
}

fn extract_python_imports(content: &str) -> Vec<String> {
    let mut imports = Vec::new();

    // from X import Y
    let from_re = Regex::new(r"(?m)^\s*from\s+([a-zA-Z_][a-zA-Z0-9_.]*)\s+import").unwrap();
    for cap in from_re.captures_iter(content) {
        if let Some(m) = cap.get(1) {
            imports.push(m.as_str().to_string());
        }
    }

    // import X
    let import_re = Regex::new(r"(?m)^\s*import\s+([a-zA-Z_][a-zA-Z0-9_.]*)").unwrap();
    for cap in import_re.captures_iter(content) {
        if let Some(m) = cap.get(1) {
            imports.push(m.as_str().to_string());
        }
    }

    imports
}

fn extract_go_imports(content: &str) -> Vec<String> {
    let mut imports = Vec::new();

    // Single import
    let single_re = Regex::new(r#"(?m)^\s*import\s+"([^"]+)""#).unwrap();
    for cap in single_re.captures_iter(content) {
        if let Some(m) = cap.get(1) {
            imports.push(m.as_str().to_string());
        }
    }

    // Block import
    let block_re = Regex::new(r#"(?ms)import\s*\((.*?)\)"#).unwrap();
    let line_re = Regex::new(r#""([^"]+)""#).unwrap();
    for cap in block_re.captures_iter(content) {
        if let Some(block) = cap.get(1) {
            for line_cap in line_re.captures_iter(block.as_str()) {
                if let Some(m) = line_cap.get(1) {
                    imports.push(m.as_str().to_string());
                }
            }
        }
    }

    imports
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rust_imports() {
        let code = r#"
use crate::db;
use crate::claims::handle;
use std::path::Path;
mod events;
"#;
        let imports = extract_rust_imports(code);
        assert!(imports.contains(&"crate::db".to_string()));
        assert!(imports.contains(&"crate::claims::handle".to_string()));
        assert!(imports.contains(&"std::path::Path".to_string()));
        assert!(imports.contains(&"mod::events".to_string()));
    }

    #[test]
    fn test_js_imports() {
        let code = r#"
import { useState } from 'react';
import App from './App';
const fs = require('fs');
"#;
        let imports = extract_js_imports(code);
        assert!(imports.contains(&"react".to_string()));
        assert!(imports.contains(&"./App".to_string()));
        assert!(imports.contains(&"fs".to_string()));
    }

    #[test]
    fn test_python_imports() {
        let code = r#"
import os
from pathlib import Path
from mypackage.module import something
"#;
        let imports = extract_python_imports(code);
        assert!(imports.contains(&"os".to_string()));
        assert!(imports.contains(&"pathlib".to_string()));
        assert!(imports.contains(&"mypackage.module".to_string()));
    }
}
