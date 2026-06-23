mod cli;
mod detect;
mod diff;
mod imports;
mod history;
mod conventions;
mod analyze;
mod report;

use clap::Parser;
use cli::{Cli, Command};

fn main() {
    let cli = Cli::parse();
    let result = run(&cli);
    match result {
        Ok(()) => {}
        Err(e) => {
            let code = e.exit_code();
            if cli.is_json() {
                let err_json = serde_json::json!({
                    "ok": false,
                    "error": {
                        "code": e.error_code(),
                        "message": e.to_string(),
                    }
                });
                eprintln!("{}", serde_json::to_string_pretty(&err_json).unwrap_or_else(|_| format!("{{\"ok\":false,\"error\":{{\"message\":\"{e}\"}}}}")));
            } else {
                eprintln!("error: {e}");
            }
            std::process::exit(code);
        }
    }
}

fn run(cli: &Cli) -> Result<(), SieveError> {
    match &cli.command {
        Command::Analyze { base, file } => {
            let repo = cli.resolve_repo()?;
            let changed_files = diff::get_changed_files(&repo, base.as_deref(), file)?;

            if changed_files.is_empty() {
                if cli.is_json() {
                    println!("{}", serde_json::to_string_pretty(&serde_json::json!({
                        "ok": true,
                        "message": "No changed files detected",
                        "signals": [],
                        "full_validation_command": null,
                    }))?);
                } else {
                    println!("No changed files detected.");
                }
                return Ok(());
            }

            let projects = detect::detect_projects(&repo);
            let result = analyze::run_analysis(&repo, &changed_files, &projects)?;
            report::print_analysis(&result, cli.is_json())?;
            Ok(())
        }
        Command::Files { base, file } => {
            let repo = cli.resolve_repo()?;
            let changed_files = diff::get_changed_files(&repo, base.as_deref(), file)?;
            if cli.is_json() {
                println!("{}", serde_json::to_string_pretty(&serde_json::json!({
                    "ok": true,
                    "files": changed_files,
                }))?);
            } else {
                for f in &changed_files {
                    println!("{f}");
                }
            }
            Ok(())
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SieveError {
    #[error("{0}")]
    Validation(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
}

impl SieveError {
    pub fn exit_code(&self) -> i32 {
        match self {
            SieveError::Validation(_) => 1,
            SieveError::Io(_) => 2,
            SieveError::Json(_) => 1,
        }
    }

    pub fn error_code(&self) -> &'static str {
        match self {
            SieveError::Validation(_) => "validation_error",
            SieveError::Io(_) => "io_error",
            SieveError::Json(_) => "json_error",
        }
    }
}
