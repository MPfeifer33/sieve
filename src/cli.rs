use clap::{Parser, Subcommand};
use std::path::PathBuf;

use crate::SieveError;

#[derive(Parser, Debug)]
#[command(name = "sieve", version, about = "Test impact analyzer — recommends minimal test sets from git diffs")]
pub struct Cli {
    /// Project root override
    #[arg(long, global = true)]
    pub repo: Option<PathBuf>,

    /// Output format
    #[arg(long, global = true, default_value = "text")]
    pub format: OutputFormat,

    #[command(subcommand)]
    pub command: Command,
}

impl Cli {
    pub fn resolve_repo(&self) -> Result<PathBuf, SieveError> {
        if let Some(ref repo) = self.repo {
            return Ok(repo.clone());
        }
        if let Ok(output) = std::process::Command::new("git")
            .args(["rev-parse", "--show-toplevel"])
            .output()
        {
            if output.status.success() {
                let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
                return Ok(PathBuf::from(path));
            }
        }
        std::env::current_dir().map_err(SieveError::Io)
    }

    pub fn is_json(&self) -> bool {
        matches!(self.format, OutputFormat::Json)
    }
}

#[derive(Debug, Clone, clap::ValueEnum)]
pub enum OutputFormat {
    Json,
    Text,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Analyze changed files and recommend tests to run
    Analyze {
        /// Base revision to diff against (default: HEAD for unstaged, or staged changes)
        #[arg(long)]
        base: Option<String>,
        /// Explicit file paths to analyze (instead of git diff)
        #[arg(long)]
        file: Vec<String>,
    },
    /// List changed files without analysis (useful for debugging)
    Files {
        /// Base revision
        #[arg(long)]
        base: Option<String>,
        /// Explicit file paths
        #[arg(long)]
        file: Vec<String>,
    },
}
