//! One malformed trace costs its own row and nothing else.

mod common;

use common::{
    bound_request, committed_format, copy_committed_fixtures, declared_pattern, registry,
    synthetic_format, tempdir,
};
use plateforce_batch::{analyse, BatchRequest, TrialIdentity, TrialSet};

#[test]
fn one_bad_trial_costs_one_row_and_the_run_continues() {
    let directory = tempdir("one-bad-trial");
    let copied = copy_committed_fixtures(&directory);
    assert!(copied >= 6, "the committed traces are the good half");
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

    assert_eq!(
        result.results.len(),
        copied + 1,
        "every trial keeps its row"
    );
    assert_eq!(result.coverage.trial_count, copied + 1);
    assert_eq!(result.coverage.computed, copied);
    assert_eq!(result.coverage.refused, 1);
    assert_eq!(
        result
            .results
            .iter()
            .filter(|row| row.refusal_code.is_empty())
            .count(),
        copied,
        "every good trace carries values"
    );

    let refused: Vec<&str> = result
        .results
        .iter()
        .filter(|row| !row.refusal_code.is_empty())
        .map(|row| row.trial_id.as_str())
        .collect();
    assert_eq!(refused, vec!["truncated"], "the bad trial fails by name");
    assert!(
        result
            .refusals
            .iter()
            .any(|row| row.trial_id == "truncated"),
        "and it says why"
    );
    result
        .run
        .check_invariants()
        .expect("the run keeps its own arithmetic");
    std::fs::remove_dir_all(&directory).ok();
}

/// `refusals` is keyed by trial and ordinal rather than by trial alone, because a trial that
/// computed some numbers and declined a landmark carries several rows. Keying it by trial
/// would let one decline overwrite another and report fewer than happened.
#[test]
fn a_trial_that_declined_more_than_once_keeps_a_row_for_each_decline() {
    let directory = tempdir("refusal-ordinals");
    plateforce_batch::synthetic::write_corpus(&directory, 2, 2, 7).unwrap();
    // Two names the declared template cannot parse, so both refuse under one trial id.
    for name in ["notes.txt", "second_note.txt"] {
        std::fs::write(directory.join(name), "0\n0\n0\n").unwrap();
    }
    let set = TrialSet::walk(&directory, &synthetic_format(), &declared_pattern()).unwrap();
    let result = analyse(&set, &bound_request(), &registry()).expect("every choice was made");

    let mut keys: Vec<(&str, usize)> = result
        .refusals
        .iter()
        .map(|row| (row.trial_id.as_str(), row.ordinal))
        .collect();
    let rows = keys.len();
    keys.sort_unstable();
    keys.dedup();
    println!(
        "refusal rows {rows}, distinct trial and ordinal pairs {}",
        keys.len()
    );
    assert_eq!(
        keys.len(),
        rows,
        "trial and ordinal together key the relation"
    );

    let busiest = result
        .refusals
        .iter()
        .map(|row| {
            result
                .refusals
                .iter()
                .filter(|other| other.trial_id == row.trial_id)
                .count()
        })
        .max()
        .unwrap_or(0);
    assert!(
        busiest >= 2,
        "one id carried {busiest} rows, so the ordinal was never asked to do anything"
    );
    std::fs::remove_dir_all(&directory).ok();
}

/// The partial state, and the reason refusals are keyed by trial and ordinal rather than by
/// trial. A trace that weighs and never crosses either threshold computes what it can and
/// declines two landmarks, and both declines have to survive in the same relation.
#[test]
fn a_trial_that_computed_and_declined_two_landmarks_carries_a_row_for_each() {
    let directory = tempdir("partial-state");
    // Stands on the plate for three seconds and never leaves it. The ripple is deterministic
    // so the noise-relative onset threshold sits just above the highest sample.
    let standing: String = (0..3600)
        .map(|sample| format!("{}\n", 700.0 + ((sample % 7) as f64) * 0.05))
        .collect();
    std::fs::write(directory.join("standing.force.txt"), standing).unwrap();

    let set = TrialSet::walk(&directory, &committed_format(), &TrialIdentity::FileStem).unwrap();
    let result = analyse(&set, &bound_request(), &registry()).expect("every choice was made");

    let row = result
        .results
        .iter()
        .find(|row| row.trial_id == "standing")
        .expect("the trial keeps its row");
    let computed = row.values.values().filter(|value| value.is_some()).count();
    let mut declines: Vec<(usize, &str)> = result
        .refusals
        .iter()
        .filter(|refusal| refusal.trial_id == "standing")
        .map(|refusal| (refusal.ordinal, refusal.slot.as_str()))
        .collect();
    declines.sort_unstable();
    println!("standing computed {computed} quantities and declined {declines:?}");

    assert!(computed > 0, "the trial produced numbers");
    assert!(
        row.refusal_code.is_empty(),
        "so it is not a trial that produced nothing"
    );
    assert_eq!(
        declines,
        vec![(0, "onset"), (1, "takeoff")],
        "each decline keeps its own row under its own ordinal"
    );
    std::fs::remove_dir_all(&directory).ok();
}

/// Every code a run publishes has to be one the vocabulary carries, and has to be the one
/// that names what went wrong. A file the identity pattern could not parse reported a missing
/// column until `TrialIdentityUnparsed` existed, which sent a reader looking at their columns.
#[test]
fn every_refusal_carries_a_published_code_that_names_its_fault() {
    let published: std::collections::BTreeSet<String> = plateforce_core::RefusalCode::ALL
        .iter()
        .map(|code| {
            serde_json::to_value(code)
                .ok()
                .and_then(|value| value.as_str().map(str::to_string))
                .expect("a code spells itself")
        })
        .collect();

    let directory = tempdir("published-codes");
    plateforce_batch::synthetic::write_corpus(&directory, 2, 2, 7).unwrap();
    // One name the pattern cannot read, and one it can read whose trace it cannot, so more
    // than one kind of fault is on the table.
    std::fs::write(directory.join("notes.txt"), "0\n0\n0\n").unwrap();
    std::fs::write(directory.join("AT09_1.txt"), "").unwrap();
    let set = TrialSet::walk(&directory, &synthetic_format(), &declared_pattern()).unwrap();
    let result = analyse(&set, &bound_request(), &registry()).expect("every choice was made");

    let seen: std::collections::BTreeSet<&str> = result
        .refusals
        .iter()
        .map(|row| row.code.as_str())
        .collect();
    println!("codes this run published: {seen:?}");
    assert!(!seen.is_empty(), "the run refused something");
    for code in &seen {
        assert!(
            published.contains(*code),
            "{code} is not one of the {} codes the vocabulary carries",
            published.len()
        );
    }

    let unparsed = result
        .refusals
        .iter()
        .find(|row| row.parameter == "notes.txt")
        .expect("the unparsed name keeps a row");
    assert_eq!(
        unparsed.code, "trial_identity_unparsed",
        "a name the pattern could not read is an identity fault, not a column one"
    );
    assert!(
        unparsed.available.contains("{subject}"),
        "and it names the template that would resolve it: {}",
        unparsed.available
    );
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
    assert_eq!(
        refusal.unresolved.len(),
        2,
        "two of three constructs force one"
    );
    assert!(
        refusal.message.contains("system_weight"),
        "{}",
        refusal.message
    );
    assert!(
        refusal.message.contains("movement_onset"),
        "{}",
        refusal.message
    );
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
    let copied = copy_committed_fixtures(&directory);
    // The reader accepts this and every landmark rule declines it, which is a different
    // failure point from a file that will not parse and must cost the same one row.
    std::fs::write(directory.join("stub.force.txt"), "600.0\n").unwrap();

    let set = TrialSet::walk(&directory, &committed_format(), &TrialIdentity::FileStem).unwrap();
    let result = analyse(&set, &bound_request(), &registry()).unwrap();

    println!("{}", result.coverage.line());
    assert_eq!(result.results.len(), copied + 1);
    assert_eq!(result.coverage.computed, copied);
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
    let copied = copy_committed_fixtures(&directory);
    std::fs::write(directory.join("README.md"), "not a trace\n").unwrap();
    let set = TrialSet::walk(&directory, &committed_format(), &TrialIdentity::FileStem).unwrap();
    let result = analyse(&set, &bound_request(), &registry()).unwrap();

    let line = result.coverage.line();
    println!("{line}");
    assert!(line.contains(&format!("files {copied} found")), "{line}");
    assert!(
        line.contains(&format!("computed {copied} of {copied}")),
        "{line}"
    );
    assert!(line.contains(&format!("excluded 0 of {copied}")), "{line}");
    assert_eq!(
        result.coverage.computed + result.coverage.refused,
        result.coverage.trial_count,
        "the counts sum to the denominator"
    );
    std::fs::remove_dir_all(&directory).ok();
}
