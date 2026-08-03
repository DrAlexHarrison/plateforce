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
        // As the registry names constructs, so a reader of this column can look the name up.
        vec![(0, "movement_onset"), (1, "takeoff")],
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
    let strays = ["README.md", "notes.txt", "session.log"];
    for name in strays {
        std::fs::write(directory.join(name), "not a trace\n").unwrap();
    }
    let set = TrialSet::walk(&directory, &committed_format(), &TrialIdentity::FileStem).unwrap();
    let result = analyse(&set, &bound_request(), &registry()).unwrap();

    let line = result.coverage.line();
    println!("{line}");
    let present = copied + strays.len();
    // The folder's own population, then what the declared suffixes kept, then what they
    // passed over. A line stating only the survivors reads as the whole folder.
    assert!(line.contains(&format!("files {present},")), "{line}");
    assert!(
        line.contains(&format!(
            "{copied} carrying a declared trial suffix and {} not",
            strays.len()
        )),
        "{line}"
    );
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

/// The number a reader takes away from a run, checked where they read it rather than only in
/// the struct that holds it.
///
/// A folder of traces beside a few files that are not traces is the ordinary case, and a run
/// that reported the traces alone would be describing its own narrowing as the folder's
/// contents. The counts have to survive to the record on disk, which is what another tool
/// reads, and to the read-back result, which is what a library caller holds.
#[test]
fn a_run_reports_the_files_its_declaration_passed_over() {
    let directory = tempdir("passed-over");
    let copied = copy_committed_fixtures(&directory);
    let strays = ["README.md", "protocol.pdf", "AT01_6.csv", "notes"];
    for name in strays {
        std::fs::write(directory.join(name), "not a trace\n").unwrap();
    }
    let set = TrialSet::walk(&directory, &committed_format(), &TrialIdentity::FileStem).unwrap();
    let result = analyse(&set, &bound_request(), &registry()).unwrap();

    println!("{}", result.coverage.line());
    assert_eq!(set.files_found, copied);
    assert_eq!(set.files_without_declared_suffix, strays.len());
    assert_eq!(set.files_present(), copied + strays.len());
    assert_eq!(result.coverage.files_without_declared_suffix, strays.len());
    assert_eq!(result.run.files_without_declared_suffix, strays.len());

    // The passed-over files are outside the population the identity works over, so the
    // invariant the run states about itself still holds with them counted.
    result
        .run
        .check_invariants()
        .expect("a file with no declared suffix was never a trial the identity failed to name");

    let out = tempdir("passed-over-record");
    result.write_csv(&out).expect("the relations are written");
    let record: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(out.join("run.json")).unwrap()).unwrap();
    assert_eq!(
        record["files_without_declared_suffix"],
        serde_json::json!(strays.len())
    );
    assert_eq!(record["files_found"], serde_json::json!(copied));

    let back = plateforce_batch::BatchResult::from_json(&result.to_json())
        .expect("the envelope reads back");
    assert_eq!(back.coverage.line(), result.coverage.line());

    std::fs::remove_dir_all(&directory).ok();
    std::fs::remove_dir_all(&out).ok();
}

/// A result read back off the wire reports the run it came from, not an empty one.
///
/// The library, the browser and Python all rebuild a result from this envelope, and a
/// rebuilt result that answered `files 0, 0 carrying a declared trial suffix and 0 not,
/// computed 0 of 0` would be stating a measurement nobody made, in the shape of a real one.
#[test]
fn a_result_rebuilt_from_its_envelope_reports_the_counts_it_arrived_with() {
    let directory = tempdir("envelope-coverage");
    let copied = copy_committed_fixtures(&directory);
    std::fs::write(directory.join("README.md"), "not a trace\n").unwrap();
    let set = TrialSet::walk(&directory, &committed_format(), &TrialIdentity::FileStem).unwrap();
    let result = analyse(&set, &bound_request(), &registry()).unwrap();

    let back = plateforce_batch::BatchResult::from_json(&result.to_json())
        .expect("the envelope reads back");
    println!("out {}", result.coverage.line());
    println!("in  {}", back.coverage.line());

    assert!(copied > 0, "the fixtures are on disk");
    assert_eq!(back.coverage.files_found, copied);
    assert_eq!(back.coverage.files_without_declared_suffix, 1);
    assert_eq!(back.coverage.trial_count, copied);
    assert_eq!(back.coverage.results_written, copied);
    assert_eq!(back.coverage.computed, copied);
    assert_eq!(back.coverage.refused, 0);
    assert_eq!(back.coverage, result.coverage);

    std::fs::remove_dir_all(&directory).ok();
}

/// A trace of forces carries nothing about the plate that wrote it, so a run reports every
/// trial's capture as unrecorded until a caller states one. Datasets that cannot fill the
/// block fingerprint as incomplete rather than as matching, which is the whole reason the
/// block is on the record.
#[test]
fn a_run_that_states_nothing_about_its_capture_claims_no_complete_acquisition() {
    let directory = tempdir("acquisition-unstated");
    let copied = copy_committed_fixtures(&directory);
    let set = TrialSet::walk(&directory, &committed_format(), &TrialIdentity::FileStem).unwrap();

    let silent = analyse(&set, &bound_request(), &registry()).unwrap();
    println!(
        "unstated: {} of {} trials report a complete capture",
        silent.run.acquisition_complete_count, silent.run.computed_count
    );
    assert_eq!(silent.run.acquisition_complete_count, 0);
    assert_eq!(silent.run.computed_count, copied);

    let described = analyse(
        &set,
        &bound_request().describing(plateforce_core::Acquisition {
            filter_at_capture: Some("none".to_string()),
            tare_state: Some("tared_before_trial".to_string()),
            plate_natural_frequency_hz: Some(400.0),
            floor_surface: Some("concrete".to_string()),
            firmware_version: Some("2.4.1".to_string()),
        }),
        &registry(),
    )
    .unwrap();
    println!(
        "described: {} of {} trials report a complete capture",
        described.run.acquisition_complete_count, described.run.computed_count
    );
    assert_eq!(described.run.acquisition_complete_count, copied);

    // A block missing one member is not a complete one, which is the distinction the count
    // exists to make.
    let partial = analyse(
        &set,
        &bound_request().describing(plateforce_core::Acquisition {
            filter_at_capture: Some("none".to_string()),
            tare_state: Some("tared_before_trial".to_string()),
            plate_natural_frequency_hz: Some(400.0),
            floor_surface: Some("concrete".to_string()),
            firmware_version: None,
        }),
        &registry(),
    )
    .unwrap();
    assert_eq!(
        partial.run.acquisition_complete_count, 0,
        "one member short is not complete"
    );

    // And the record tells the three runs apart.
    assert_ne!(silent.run.run_fingerprint, described.run.run_fingerprint);
    assert_ne!(described.run.run_fingerprint, partial.run.run_fingerprint);
    std::fs::remove_dir_all(&directory).ok();
}

/// Who chose each value is the record, so a value the caller stated must not be attributed
/// to the registry. The distinction lives two crates away in the resolution layer and a
/// refactor there is only caught by something asserting it downstream.
#[test]
fn a_value_the_request_stated_is_not_attributed_to_the_registry() {
    let directory = tempdir("stated-not-assumed");
    copy_committed_fixtures(&directory);
    let set = TrialSet::walk(&directory, &committed_format(), &TrialIdentity::FileStem).unwrap();
    // `bound_request` states three parameters and leaves everything else to the rules.
    let result = analyse(&set, &bound_request(), &registry()).expect("every choice was made");

    let sources_for = |parameter: &str| -> Vec<&str> {
        result
            .provenance
            .iter()
            .filter(|row| row.parameter == parameter)
            .map(|row| row.source.as_str())
            .collect()
    };
    for stated in ["duration", "k", "threshold_n"] {
        let sources = sources_for(stated);
        println!("{stated:12} recorded as {sources:?}");
        assert!(!sources.is_empty(), "{stated} reaches the record at all");
        assert!(
            sources.iter().all(|source| *source == "stated"),
            "{stated} was stated by the request and the record says {sources:?}"
        );
    }

    // Both kinds reach the record, so a run whose provenance is uniformly one word would
    // fail here. It does not prove the reverse attribution: an unstated value recorded as
    // stated is caught in the analysis crate, by the test that watches the persistence
    // operator run when nobody asked for it.
    let assumed: Vec<&str> = result
        .provenance
        .iter()
        .filter(|row| row.source == "assumed")
        .map(|row| row.parameter.as_str())
        .collect();
    println!("{} rows the rules supplied themselves", assumed.len());
    assert!(
        !assumed.is_empty(),
        "a run this short states three values and the rules supply the rest"
    );
    std::fs::remove_dir_all(&directory).ok();
}
