//! Print the conformance of the Rust core against the frozen reference run.
//!
//! The point of this command is that somebody can satisfy themselves the numbers are
//! right without reading any Rust.

use plateforce_conformance::bindings::ReferenceBindings;
use plateforce_conformance::corpus::{Corpus, CorpusFormat};
use plateforce_conformance::{compare, parse_reference, Agreement, Tolerance};
use std::path::PathBuf;
use std::process::ExitCode;

const USAGE: &str = "\
plateforce-conformance - compare the Rust core against a frozen reference run

USAGE:
    plateforce-conformance --reference <CSV> --corpus <DIR> [OPTIONS]

OPTIONS:
    --relative-tolerance <F>   default 1e-9
    --absolute-tolerance <F>   default 1e-12
    --sample-rate <HZ>         default 1200
    --all-columns              list every column, not only the ones that disagree
";

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    if arguments.iter().any(|a| a == "-h" || a == "--help") {
        print!("{USAGE}");
        return ExitCode::SUCCESS;
    }
    let value = |flag: &str| -> Option<String> {
        arguments
            .iter()
            .position(|a| a == flag)
            .and_then(|at| arguments.get(at + 1))
            .cloned()
    };
    let (Some(reference_path), Some(corpus_path)) = (value("--reference"), value("--corpus")) else {
        eprintln!("plateforce-conformance: --reference and --corpus are both required");
        eprint!("{USAGE}");
        return ExitCode::FAILURE;
    };

    let mut bindings = ReferenceBindings::default();
    if let Some(rate) = value("--sample-rate").and_then(|v| v.parse().ok()) {
        bindings.sample_rate_hz = rate;
    }
    let tolerance = Tolerance {
        relative: value("--relative-tolerance")
            .and_then(|v| v.parse().ok())
            .unwrap_or(Tolerance::default().relative),
        absolute: value("--absolute-tolerance")
            .and_then(|v| v.parse().ok())
            .unwrap_or(Tolerance::default().absolute),
    };

    let text = match std::fs::read_to_string(&reference_path) {
        Ok(text) => text,
        Err(error) => {
            eprintln!("plateforce-conformance: reading {reference_path}: {error}");
            return ExitCode::FAILURE;
        }
    };
    let reference = match parse_reference(&text) {
        Ok(rows) => rows,
        Err(error) => {
            eprintln!("plateforce-conformance: {error}");
            return ExitCode::FAILURE;
        }
    };
    let format = CorpusFormat {
        sample_rate_hz: bindings.sample_rate_hz,
        ..Default::default()
    };
    let corpus = match Corpus::open(&PathBuf::from(&corpus_path), format) {
        Ok(corpus) => corpus,
        Err(error) => {
            eprintln!("plateforce-conformance: reading corpus: {error}");
            return ExitCode::FAILURE;
        }
    };

    let report = compare(
        &reference,
        &|subject, trial| corpus.trial(subject, trial),
        &bindings,
        tolerance,
    );

    println!(
        "reference rows {}, trials compared {}, corpus files indexed {}",
        report.reference_rows,
        report.trials_compared,
        corpus.len()
    );
    println!(
        "tolerance: relative {:e}, absolute {:e}",
        tolerance.relative, tolerance.absolute
    );
    println!();

    let show_all = arguments.iter().any(|a| a == "--all-columns");
    println!(
        "{:<30} {:>5} {:>9} {:>13} {:>13}  {}",
        "column", "kind", "agreement", "worst abs", "worst rel", "worst trial"
    );
    for column in &report.columns {
        if column.is_clean() && !show_all {
            continue;
        }
        let kind = match column.agreement {
            Agreement::Exact => "exact",
            Agreement::Numeric => "num",
        };
        let trial = column
            .worst_trial
            .map(|(subject, trial)| format!("subject {subject} trial {trial}"))
            .unwrap_or_default();
        println!(
            "{:<30} {:>5} {:>4}/{:<4} {:>13.3e} {:>13.3e}  {}",
            column.name,
            kind,
            column.matched,
            column.compared,
            column.worst_absolute_difference,
            column.worst_relative_difference,
            trial
        );
    }
    if report.breaches().is_empty() {
        println!("(every column agrees; pass --all-columns to list them)");
    }
    for column in report.breaches() {
        println!();
        println!("{} disagreed on {} trial(s):", column.name, column.disagreements.len());
        for item in column.disagreements.iter().take(12) {
            println!(
                "  subject {:>2} trial {}: reference {:>18} computed {:>18}",
                item.subject, item.trial, item.reference, item.computed
            );
        }
    }

    println!();
    println!("excluded trials: {}", report.exclusions.len());
    for exclusion in &report.exclusions {
        println!(
            "  subject {} trial {}: {}",
            exclusion.subject, exclusion.trial, exclusion.reason
        );
    }
    if !report.missing_from_corpus.is_empty() {
        println!("reference rows with no corpus file: {:?}", report.missing_from_corpus);
    }
    println!();
    println!(
        "weighing window under-determined on {} of {} trials, worst tie {} windows",
        report.tied_weighing_window_trials,
        report.trials_compared,
        report.worst_tied_weighing_window_count
    );
    println!(
        "longest-run takeoff preferred a later run than the first flight phase on {} of {} trials",
        report.late_takeoff_trials, report.trials_compared
    );

    if report.is_clean() {
        ExitCode::SUCCESS
    } else {
        eprintln!();
        eprintln!(
            "plateforce-conformance: {} column(s) disagree with the reference",
            report.breaches().len()
        );
        ExitCode::FAILURE
    }
}
