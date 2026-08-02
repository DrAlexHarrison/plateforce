//! One malformed trace costs its own row and nothing else.

mod common;

use common::{bound_request, committed_format, copy_committed_fixtures, registry, tempdir};
use plateforce_batch::{analyse, BatchRequest, TrialIdentity, TrialSet};

#[test]
fn one_bad_trial_costs_one_row_and_the_run_continues() {
    let directory = tempdir("one-bad-trial");
    let copied = copy_committed_fixtures(&directory);
    assert_eq!(copied, 6, "the six committed traces are the good half");
    // Cut before any row landed, which is the failure a run over a copied folder actually
    // meets and which the reader is the one to catch.
    std::fs::write(directory.join("truncated.force.txt"), "").unwrap();

    let set = TrialSet::walk(&directory, &committed_format(), &TrialIdentity::FileStem).unwrap();
    let result = analyse(&set, &bound_request(), &registry()).expect("every choice was made");

    println!("{}", result.coverage.line());
    println!(
        "results {} of {} trials, computed {} of {}, refused {} of {}",
        result.results.len(),
        set.len(),
        result.coverage.computed,
        set.len(),
        result.coverage.refused,
        set.len()
    );

    assert_eq!(result.results.len(), 7, "every trial keeps its row");
    assert_eq!(result.coverage.trial_count, 7);
    assert_eq!(result.coverage.computed, 6);
    assert_eq!(result.coverage.refused, 1);
    assert_eq!(
        result
            .results
            .iter()
            .filter(|row| row.refusal_code.is_empty())
            .count(),
        6,
        "six rows carry values"
    );

    let refused: Vec<&str> = result
        .results
        .iter()
        .filter(|row| !row.refusal_code.is_empty())
        .map(|row| row.trial_id.as_str())
        .collect();
    assert_eq!(refused, vec!["truncated"], "the bad trial fails by name");
    assert!(
        result.refusals.iter().any(|row| row.trial_id == "truncated"),
        "and it says why"
    );
    result.run.check_invariants().expect("the run keeps its own arithmetic");
    std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn a_run_with_a_choice_still_open_reads_no_trial_at_all() {
    let directory = tempdir("unresolved-precondition");
    copy_committed_fixtures(&directory);
    let set = TrialSet::walk(&directory, &committed_format(), &TrialIdentity::FileStem).unwrap();

    // The same request, with nothing recorded as deliberately chosen.
    let request = BatchRequest::new(common::analysis_request(1.0));
    let refusal = analyse(&set, &request, &registry()).expect_err("a choice is still open");

    println!("{}", refusal.message);
    assert_eq!(refusal.unresolved.len(), 2, "two of three constructs force one");
    assert!(refusal.message.contains("system_weight"), "{}", refusal.message);
    assert!(refusal.message.contains("movement_onset"), "{}", refusal.message);
    assert!(
        refusal
            .unresolved
            .iter()
            .all(|open| !open.published_alternatives.is_empty()),
        "each names what could be bound instead"
    );
    std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn a_trace_no_rule_can_work_with_also_costs_one_row() {
    let directory = tempdir("too-short");
    copy_committed_fixtures(&directory);
    // The reader accepts this and every landmark rule declines it, which is a different
    // failure point from a file that will not parse and must cost the same one row.
    std::fs::write(directory.join("stub.force.txt"), "600.0\n").unwrap();

    let set = TrialSet::walk(&directory, &committed_format(), &TrialIdentity::FileStem).unwrap();
    let result = analyse(&set, &bound_request(), &registry()).unwrap();

    println!("{}", result.coverage.line());
    assert_eq!(result.results.len(), 7);
    assert_eq!(result.coverage.computed, 6);
    assert_eq!(result.coverage.refused, 1);
    let row = result
        .results
        .iter()
        .find(|row| row.trial_id == "stub")
        .expect("the short trace keeps its row");
    assert!(!row.refusal_code.is_empty(), "and it carries a code");
    std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn the_coverage_line_names_every_count_against_one_denominator() {
    let directory = tempdir("coverage-line");
    copy_committed_fixtures(&directory);
    std::fs::write(directory.join("README.md"), "not a trace\n").unwrap();
    let set = TrialSet::walk(&directory, &committed_format(), &TrialIdentity::FileStem).unwrap();
    let result = analyse(&set, &bound_request(), &registry()).unwrap();

    let line = result.coverage.line();
    println!("{line}");
    assert!(line.contains("files 6 found"), "{line}");
    assert!(line.contains("computed 6 of 6"), "{line}");
    assert!(line.contains("excluded 0 of 6"), "{line}");
    assert_eq!(
        result.coverage.computed + result.coverage.refused,
        result.coverage.trial_count,
        "the counts sum to the denominator"
    );
    std::fs::remove_dir_all(&directory).ok();
}
