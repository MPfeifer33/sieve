use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;

use crate::analyze::{AnalysisResult, Signal};
use crate::SieveError;

#[derive(Debug, Serialize)]
struct Recommendation {
    command: String,
    confidence: String,
    score: u32,
    test_files: Vec<String>,
    changed_files: Vec<String>,
    reasons: Vec<String>,
    signal_kinds: Vec<String>,
}

#[derive(Debug, Serialize)]
struct CoverageGap {
    changed_file: String,
    reason: String,
}

#[derive(Default)]
struct RecommendationBuilder {
    command: String,
    confidence_rank: u8,
    score: u32,
    test_files: BTreeSet<String>,
    changed_files: BTreeSet<String>,
    reasons: BTreeSet<String>,
    signal_kinds: BTreeSet<String>,
}

pub fn print_analysis(result: &AnalysisResult, is_json: bool) -> Result<(), SieveError> {
    let recommendations = build_recommendations(&result.signals);
    let coverage_gaps = build_coverage_gaps(result);

    if is_json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "ok": true,
                "changed_files": result.changed_files,
                "recommendations": recommendations,
                "coverage_gaps": coverage_gaps,
                "signals": result.signals,
                "full_validation_command": result.full_validation_command,
            }))?
        );
    } else {
        print_text(result, &recommendations, &coverage_gaps);
    }
    Ok(())
}

fn build_recommendations(signals: &[Signal]) -> Vec<Recommendation> {
    let mut by_command: BTreeMap<String, RecommendationBuilder> = BTreeMap::new();

    for signal in signals {
        let Some(command) = signal.command.as_ref() else {
            continue;
        };
        let builder = by_command
            .entry(command.clone())
            .or_insert_with(|| RecommendationBuilder {
                command: command.clone(),
                ..Default::default()
            });

        let confidence_rank = confidence_rank(&signal.confidence);
        builder.confidence_rank = builder.confidence_rank.max(confidence_rank);
        builder.score += confidence_score(&signal.confidence) + signal_score(&signal.kind);
        builder.changed_files.insert(signal.changed_file.clone());
        builder.reasons.insert(signal.reason.clone());
        builder.signal_kinds.insert(signal.kind.clone());
        if let Some(test_file) = &signal.test_file {
            builder.test_files.insert(test_file.clone());
        }
    }

    let mut recommendations: Vec<Recommendation> = by_command
        .into_values()
        .map(|builder| Recommendation {
            command: builder.command,
            confidence: confidence_label(builder.confidence_rank).to_string(),
            score: builder.score,
            test_files: builder.test_files.into_iter().collect(),
            changed_files: builder.changed_files.into_iter().collect(),
            reasons: builder.reasons.into_iter().collect(),
            signal_kinds: builder.signal_kinds.into_iter().collect(),
        })
        .collect();

    recommendations.sort_by(|a, b| {
        b.score
            .cmp(&a.score)
            .then_with(|| confidence_rank(&b.confidence).cmp(&confidence_rank(&a.confidence)))
            .then_with(|| a.command.cmp(&b.command))
    });

    recommendations
}

fn build_coverage_gaps(result: &AnalysisResult) -> Vec<CoverageGap> {
    result
        .changed_files
        .iter()
        .filter(|changed_file| {
            !result
                .signals
                .iter()
                .any(|signal| signal.changed_file == **changed_file && signal.test_file.is_some())
        })
        .map(|changed_file| CoverageGap {
            changed_file: changed_file.clone(),
            reason: "No targeted test file found; using project-level fallback if available"
                .to_string(),
        })
        .collect()
}

fn print_text(
    result: &AnalysisResult,
    recommendations: &[Recommendation],
    coverage_gaps: &[CoverageGap],
) {
    println!("sieve: {} changed file(s)", result.changed_files.len());
    println!();

    if recommendations.is_empty() {
        println!("  No test recommendations found.");
    } else {
        println!("  Recommendations:");
        for recommendation in recommendations {
            println!(
                "    [{} score={}] {}",
                recommendation.confidence, recommendation.score, recommendation.command
            );
            if !recommendation.test_files.is_empty() {
                println!("      tests: {}", recommendation.test_files.join(", "));
            }
            if !recommendation.changed_files.is_empty() {
                println!("      changes: {}", recommendation.changed_files.join(", "));
            }
            for reason in &recommendation.reasons {
                println!("      reason: {reason}");
            }
        }
    }

    if !coverage_gaps.is_empty() {
        println!();
        println!("  Coverage gaps:");
        for gap in coverage_gaps {
            println!("    {}: {}", gap.changed_file, gap.reason);
        }
    }

    if let Some(ref full_cmd) = result.full_validation_command {
        println!();
        println!("  Full validation: `{full_cmd}`");
    }

    if !result.signals.is_empty() {
        println!();
        println!("  Raw signals: {}", result.signals.len());
    }
}

fn confidence_rank(confidence: &str) -> u8 {
    match confidence {
        "certain" => 4,
        "high" => 3,
        "medium" => 2,
        "low" => 1,
        _ => 0,
    }
}

fn confidence_label(rank: u8) -> &'static str {
    match rank {
        4 => "certain",
        3 => "high",
        2 => "medium",
        1 => "low",
        _ => "unknown",
    }
}

fn confidence_score(confidence: &str) -> u32 {
    match confidence {
        "certain" => 400,
        "high" => 300,
        "medium" => 200,
        "low" => 100,
        _ => 0,
    }
}

fn signal_score(kind: &str) -> u32 {
    match kind {
        "direct_test_change" => 50,
        "convention_match" => 40,
        "import_match" => 40,
        "history_correlation" => 20,
        "fallback" => 0,
        _ => 0,
    }
}
