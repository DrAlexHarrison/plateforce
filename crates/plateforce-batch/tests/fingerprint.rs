//! Did every trial in this batch run the same way.

mod common;

use common::{
    analysis_request, bound_request, bound_request_describing_the_plate, committed_format,
    copy_committed_fixtures, registry, tempdir,
};
use plateforce_batch::{analyse, BatchRequest, ProvenanceRow, TrialIdentity, TrialSet};
use std::collections::BTreeSet;

fn distinct_provenance(rows: &[plateforce_batch::ResultRow]) -> BTreeSet<&str> {
    rows.iter()
        .filter(|row| !row.provenance_id.is_empty())
        .map(|row| row.provenance_id.as_str())
        .collect()
}

#[test]
fn trials_that_produced_the_same_quantities_under_the_same_rules_share_one_chain() {
    let directory = tempdir("fingerprint-same");
    let copied = copy_committed_fixtures(&directory);
    let set = TrialSet::walk(&directory, &committed_format(), &TrialIdentity::FileStem).unwrap();
    let result = analyse(&set, &bound_request(), &registry()).unwrap();

    // Asserted before anything else, because every claim below is true of an empty run and
    // a suite that passes on a tree where nothing computed has proved nothing.
    assert_eq!(
        result.coverage.computed, copied,
        "{} of {copied} trials computed",
        result.coverage.computed
    );

    let distinct = distinct_provenance(&result.results);
    println!(
        "distinct provenance_id: {} of {copied} trials",
        distinct.len()
    );
    assert!(!distinct.is_empty(), "a computed trial carries a chain");
    assert_eq!(
        distinct.len(),
        result.run.distinct_provenance_count,
        "the run states the count rather than leaving it to a diff"
    );
    assert!(
        distinct.iter().all(|id| id.starts_with("content-")),
        "the digest is the one already in the tree"
    );

    // Two chains over six committed traces, and the reason is a fact about the traces rather
    // than about the request: the flight-time route needs a touchdown inside the recording,
    // and only one of the six carries one. So five trials produced nine quantities and one
    // produced eleven, which is not the same analysis and does not claim to be.
    let grouped: BTreeSet<usize> = result
        .results
        .iter()
        .filter(|row| !row.provenance_id.is_empty())
        .map(|row| row.values.values().filter(|value| value.is_some()).count())
        .collect();
    println!(
        "{} distinct quantity counts across {copied} trials: {grouped:?}",
        grouped.len()
    );
    assert_eq!(
        distinct.len(),
        grouped.len(),
        "a chain per set of quantities actually produced"
    );
    std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn a_trial_that_ran_differently_gets_its_own_chain() {
    let directory = tempdir("fingerprint-differs");
    let copied = copy_committed_fixtures(&directory);
    let set = TrialSet::walk(&directory, &committed_format(), &TrialIdentity::FileStem).unwrap();
    let registry = registry();

    // The whole set under one request, then the same set under a different weighing window.
    // Two runs stand in for the per-trial override the relation is designed to keep visible.
    //
    // Both describe the plate, because a run whose acquisition block is unfilled publishes no
    // fingerprint at all, and two runs withholding theirs would satisfy the inequality below
    // without either digest being read.
    let first = analyse(&set, &bound_request_describing_the_plate(), &registry).unwrap();
    let overridden = BatchRequest::new(analysis_request(0.5))
        .resolving(&["system_weight", "movement_onset", "takeoff"])
        .describing(common::a_recorded_plate());
    let second = analyse(&set, &overridden, &registry).unwrap();

    assert_eq!(
        first.coverage.computed, copied,
        "the first run computed every trial"
    );
    assert_eq!(second.coverage.computed, copied, "and so did the second");

    let before = distinct_provenance(&first.results);
    let mut combined = before.clone();
    combined.extend(distinct_provenance(&second.results));
    println!(
        "distinct provenance_id: {} of {copied} trials, then {} of {copied} across both runs",
        before.len(),
        combined.len()
    );
    assert!(
        combined.len() > before.len(),
        "a changed window is a chain the first run did not carry"
    );
    assert!(
        combined.is_superset(&before),
        "and the chains the first run had are still the same chains"
    );
    assert_ne!(
        first.run.request_digest, second.run.request_digest,
        "the request that produced it is a different request"
    );
    assert!(
        first.run.run_fingerprint.is_some() && second.run.run_fingerprint.is_some(),
        "both runs described the plate, so both published a fingerprint to compare"
    );
    assert_ne!(
        first.run.run_fingerprint, second.run.run_fingerprint,
        "so the run fingerprints differently"
    );
    std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn the_chain_collapses_rather_than_repeating_itself_per_trial() {
    let directory = tempdir("fingerprint-collapse");
    let copied = copy_committed_fixtures(&directory);
    let set = TrialSet::walk(&directory, &committed_format(), &TrialIdentity::FileStem).unwrap();
    let result = analyse(&set, &bound_request(), &registry()).unwrap();

    assert_eq!(result.coverage.computed, copied, "every trial computed");
    let per_trial_if_repeated = result.provenance.len() * copied;
    println!(
        "provenance {} rows for {copied} trials, against {per_trial_if_repeated} if each trial carried its own",
        result.provenance.len()
    );
    assert!(!result.provenance.is_empty(), "the chain is recorded");
    assert!(
        result.provenance.len() < per_trial_if_repeated,
        "keying on the digest is what collapses it"
    );
    std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn every_number_reaches_the_rules_that_produced_it() {
    let directory = tempdir("fingerprint-reaches");
    copy_committed_fixtures(&directory);
    let set = TrialSet::walk(&directory, &committed_format(), &TrialIdentity::FileStem).unwrap();
    let result = analyse(&set, &bound_request(), &registry()).unwrap();

    assert!(result.coverage.computed > 0, "a trial computed");
    let row = result
        .results
        .iter()
        .find(|row| !row.provenance_id.is_empty())
        .expect("a trial computed");
    let chain: Vec<&ProvenanceRow> = result
        .provenance
        .iter()
        .filter(|entry| entry.provenance_id == row.provenance_id)
        .collect();

    // Every quantity that carries a value is reachable from the chain by name.
    let quantities_with_values: BTreeSet<&String> = row
        .values
        .iter()
        .filter(|(_, value)| value.is_some())
        .map(|(name, _)| name)
        .collect();
    let quantities_in_chain: BTreeSet<&String> =
        chain.iter().map(|entry| &entry.quantity).collect();
    println!(
        "{} of {} computed quantities reach a chain",
        quantities_with_values
            .iter()
            .filter(|name| quantities_in_chain.contains(**name))
            .count(),
        quantities_with_values.len()
    );
    assert!(
        quantities_with_values.is_subset(&quantities_in_chain),
        "a number with no chain names nothing that produced it"
    );

    // The jump height names both the arithmetic and the landmarks under it.
    let height: Vec<&&ProvenanceRow> = chain
        .iter()
        .filter(|entry| entry.quantity == "jump_height_from_takeoff_meters")
        .collect();
    assert!(
        height
            .iter()
            .any(|entry| entry.depth == 0
                && entry.method_id == "jumpheight.takeoff.impulse_momentum"),
        "the arithmetic sits at depth 0"
    );
    assert!(
        height.iter().any(|entry| entry.depth > 0),
        "and the landmark rules sit below it"
    );
    std::fs::remove_dir_all(&directory).ok();
}
