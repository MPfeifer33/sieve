use crate::analyze::AnalysisResult;
use crate::SieveError;

pub fn print_analysis(result: &AnalysisResult, is_json: bool) -> Result<(), SieveError> {
    if is_json {
        println!("{}", serde_json::to_string_pretty(&serde_json::json!({
            "ok": true,
            "changed_files": result.changed_files,
            "signals": result.signals,
            "full_validation_command": result.full_validation_command,
        }))?);
    } else {
        print_text(result);
    }
    Ok(())
}

fn print_text(result: &AnalysisResult) {
    println!("sieve: {} changed file(s)", result.changed_files.len());
    println!();

    if result.signals.is_empty() {
        println!("  No test signals found.");
        return;
    }

    // Group by confidence
    let certain: Vec<_> = result.signals.iter().filter(|s| s.confidence == "certain").collect();
    let high: Vec<_> = result.signals.iter().filter(|s| s.confidence == "high").collect();
    let medium: Vec<_> = result.signals.iter().filter(|s| s.confidence == "medium").collect();
    let low: Vec<_> = result.signals.iter().filter(|s| s.confidence == "low").collect();

    if !certain.is_empty() {
        println!("  Certain:");
        for s in &certain {
            println!("    {} → {}",
                s.changed_file,
                s.command.as_deref().unwrap_or("(no command)"),
            );
            println!("      {}", s.reason);
        }
        println!();
    }

    if !high.is_empty() {
        println!("  High confidence:");
        for s in &high {
            println!("    {} → {}",
                s.changed_file,
                s.command.as_deref().unwrap_or("(no command)"),
            );
            println!("      {}", s.reason);
        }
        println!();
    }

    if !medium.is_empty() {
        println!("  Medium confidence:");
        for s in &medium {
            println!("    {} → {}",
                s.changed_file,
                s.command.as_deref().unwrap_or("(no command)"),
            );
            println!("      {}", s.reason);
        }
        println!();
    }

    if !low.is_empty() {
        println!("  Fallback:");
        for s in &low {
            println!("    {}", s.command.as_deref().unwrap_or("(no command)"));
            println!("      {}", s.reason);
        }
        println!();
    }

    if let Some(ref full_cmd) = result.full_validation_command {
        println!("  Full validation: `{full_cmd}`");
    }
}
