//! A request that binds a rule gets an answer for every quantity that rule declares, whether
//! the rule found a number or declined.
//!
//! A rule that declined used to report no metric at all, so the quantity a caller asked for was
//! absent from the response rather than present and empty. A reader filtering a table by that
//! column met a run that answered a question they had not asked, and nothing anywhere said the
//! column had been asked for. The same absence reached the account block, which went nine keys
//! to the terminal's eleven on five of the six committed fixtures.
//!
//! Two halves, because two mechanisms produced the same absence. The account block dropped
//! every quantity with no value. The phase that runs rules computed from the landmarks iterated
//! what a rule reported rather than what its row declares, and a rule that declines reports
//! nothing.
//!
//! The counts here are paired, never taken alone. Eleven entries in a block is the same number
//! whether nine of them are accounts and two are blanks or all eleven are accounts, so every
//! count of entries is stated beside a count of non-empty ones and a count opening with a
//! value.

use std::collections::{BTreeMap, BTreeSet};

use plateforce_analysis::{
    accounts_of, recorded_number_text, run, AnalysisRequest, AnalysisResponse, MethodChoice,
    WeighingChoice,
};
use plateforce_core::provenance::RegistryStamp;
use plateforce_core::{read_trial_from_path, Trial};

mod common;

const FIXTURES: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../plateforce-conformance/fixtures/"
);
const INTERRUPTED: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../plateforce-conformance/damaged/subject01_trial1_interrupted.force.txt"
);

const CORPUS_SAMPLE_RATE_HZ: f64 = 1200.0;

/// Every recording this repository commits, and the damaged one beside them.
///
/// Nine rather than eight. A guard whose population is the eight on which every landmark places
/// cannot reach a recording where a landmark rule declines, and that is exactly the recording
/// the dropped rows were measured on.
fn every_committed_recording() -> Vec<(String, Trial)> {
    let mut recordings: Vec<(String, Trial)> = (1..=6)
        .map(|number| {
            let name = format!("subject01_trial{number}");
            let trial = trial_at(&format!("{FIXTURES}{name}.force.txt"));
            (name, trial)
        })
        .collect();
    for name in [
        "synthetic_untrimmed_step_off",
        "synthetic_untrimmed_step_off_after_jump",
    ] {
        recordings.push((
            name.to_string(),
            trial_at(&format!("{FIXTURES}{name}.force.txt")),
        ));
    }
    recordings.push((
        "subject01_trial1_interrupted".to_string(),
        trial_at(INTERRUPTED),
    ));
    recordings
}

fn trial_at(path: &str) -> Trial {
    let (trial, _) = read_trial_from_path(path, '\t', 0, CORPUS_SAMPLE_RATE_HZ)
        .unwrap_or_else(|error| panic!("{path} did not read: {error}"));
    trial
}

fn spine_request() -> AnalysisRequest {
    common::prepared(AnalysisRequest {
        weighing: WeighingChoice {
            method_id: "bwepoch.fixed_window".into(),
            parameters: BTreeMap::from([("duration".to_string(), 1.0)]),
            ..Default::default()
        },
        onset: MethodChoice {
            method_id: "onset.threshold.noise_relative".into(),
            parameters: BTreeMap::from([("k".to_string(), 5.0)]),
            ..Default::default()
        },
        takeoff: MethodChoice {
            method_id: "takeoff.threshold.absolute_force".into(),
            parameters: BTreeMap::from([("threshold_n".to_string(), 20.0)]),
            ..Default::default()
        },
        ..Default::default()
    })
}

fn binding(construct: &str, method_id: &str) -> AnalysisRequest {
    let mut request = spine_request();
    request.derived.insert(
        construct.to_string(),
        MethodChoice {
            method_id: method_id.to_string(),
            ..Default::default()
        },
    );
    common::prepared(request)
}

fn stamp() -> RegistryStamp {
    RegistryStamp {
        version: Some("fixture-pin".to_string()),
        declared_version: Some("fixture-declares".to_string()),
        digest: Some("content-fixture".to_string()),
    }
}

/// The three counts a block has to be read by. Entries alone cannot tell eleven accounts from
/// nine accounts and two blanks.
struct BlockCounts {
    entries: usize,
    non_empty: usize,
    opening_with_a_value: usize,
    metrics: usize,
}

fn counts(response: &AnalysisResponse) -> BlockCounts {
    let block = accounts_of(response, &stamp(), false);
    let opening_with_a_value = response
        .metrics
        .iter()
        .filter(|metric| {
            metric.value.is_some_and(|value| {
                block
                    .get(&metric.key)
                    .is_some_and(|account| account.starts_with(&recorded_number_text(value)))
            })
        })
        .count();
    BlockCounts {
        entries: block.len(),
        non_empty: block.values().filter(|account| !account.is_empty()).count(),
        opening_with_a_value,
        metrics: response.metrics.len(),
    }
}

/// A rule computed from the landmarks that declined still reports the quantity it was bound
/// for.
///
/// `phase.braking_start.min_force` needs both landmarks and the interrupted recording places
/// neither, so the rule declines for want of what the onset rule did not produce. Before this
/// the response carried eleven metrics on that request and the caller's twelfth column was
/// absent with nothing saying it had been asked for.
///
/// The control is the same request on the recording where the rule answers, so the assertion is
/// about the decline and not about the request: the count is the same and there is a number in
/// it.
#[test]
fn a_derived_rule_that_declined_reports_its_quantity_rather_than_no_metric_at_all() {
    let request = binding("braking_phase_start", "phase.braking_start.min_force");
    let key = "braking_phase_start_seconds";

    let declined = run(&trial_at(INTERRUPTED), &request).expect("the request is well formed");
    let metric = declined
        .metric(key)
        .unwrap_or_else(|| panic!("{key} is absent from the response the caller asked for it in"));
    assert!(metric.value.is_none(), "{:?}", metric.value);
    assert!(
        declined
            .refusals
            .iter()
            .any(|rule| rule.method_id == "phase.braking_start.min_force"),
        "the rule no longer declines on this recording, so this guard reaches no decline"
    );

    let answered = run(
        &trial_at(&format!("{FIXTURES}subject01_trial1.force.txt")),
        &request,
    )
    .expect("the request is well formed");
    let control = answered
        .metric(key)
        .unwrap_or_else(|| panic!("{key} is absent on the recording where the rule answers"));
    assert!(
        control.value.is_some(),
        "the control recording no longer answers, so the pair proves nothing"
    );

    println!(
        "{} metrics where the rule declined, {} where it answered, {key} present in both",
        declined.metrics.len(),
        answered.metrics.len()
    );
    assert_eq!(
        declined.metrics.len(),
        answered.metrics.len(),
        "the decline costs the caller a column"
    );
}

/// Every quantity every rule this build ships declares reaches the response when that rule is
/// bound.
///
/// One rule at a time over one recording, which is the recording where the landmarks are absent
/// and every rule that reads one has to decline. The denominator is the binding table's own
/// count rather than a number written here, so a rule added to the build is covered the day it
/// is added.
#[test]
fn every_quantity_a_bound_rule_declares_reaches_the_response() {
    let trial = trial_at(INTERRUPTED);
    let mut bound = 0usize;
    let mut declared_total = 0usize;
    let mut missing: Vec<String> = Vec::new();

    for row in plateforce_analysis::binding::derived_bindings() {
        let request = binding(row.construct, row.id);
        let Ok(response) = run(&trial, &request) else {
            continue;
        };
        bound += 1;
        let reported: BTreeSet<&str> = response
            .metrics
            .iter()
            .map(|metric| metric.key.as_str())
            .collect();
        for quantity in row.quantities {
            declared_total += 1;
            if !reported.contains(quantity.key) {
                missing.push(format!(
                    "{} declares {} and reports none",
                    row.id, quantity.key
                ));
            }
        }
    }

    println!("{bound} rules bound, {declared_total} declared quantities checked");
    assert!(
        bound >= 40,
        "only {bound} rules ran, which is not the build's derived table"
    );
    assert!(
        declared_total > bound,
        "every rule declares one quantity at most, so a rule declaring two is not covered here"
    );
    assert!(
        missing.is_empty(),
        "a caller bound a rule and the column it declares is absent: {missing:?}"
    );
}

/// The account block holds one entry per metric, on every recording this repository commits.
///
/// The count of entries is stated with the two counts that make it readable. A build that
/// dropped every valueless quantity reports the same eleven on `subject01_trial1`, where all
/// eleven answer, and nine on the five that stop in flight, which is why the pair below is
/// named rather than left to a loop.
#[test]
fn the_account_block_holds_one_entry_per_metric_on_every_committed_recording() {
    let mut shortfalls: Vec<String> = Vec::new();
    for (name, trial) in every_committed_recording() {
        let response = run(&trial, &spine_request()).expect("the request is well formed");
        let counted = counts(&response);
        println!(
            "{name}: {} metrics, {} entries, {} non-empty, {} opening with a value",
            counted.metrics, counted.entries, counted.non_empty, counted.opening_with_a_value
        );
        if counted.entries != counted.metrics {
            shortfalls.push(format!(
                "{name} reports {} quantities and describes {}",
                counted.metrics, counted.entries
            ));
        }
    }
    assert!(shortfalls.is_empty(), "{shortfalls:?}");
}

/// The discriminating pair for the guard above, named rather than trusted to a loop.
///
/// `subject01_trial1` answers all eleven, so it reports eleven entries under a build that drops
/// valueless quantities and under one that does not. `subject01_trial2` stops while the athlete
/// is still in the air, so two of its eleven have no number and it is the one of the two that
/// can tell the builds apart.
#[test]
fn the_recording_that_answers_everything_cannot_tell_the_two_builds_apart_and_its_sibling_can() {
    let answered = run(
        &trial_at(&format!("{FIXTURES}subject01_trial1.force.txt")),
        &spine_request(),
    )
    .expect("the request is well formed");
    let partial = run(
        &trial_at(&format!("{FIXTURES}subject01_trial2.force.txt")),
        &spine_request(),
    )
    .expect("the request is well formed");

    let full = counts(&answered);
    let some = counts(&partial);
    println!(
        "trial1 {} entries, {} opening with a value; trial2 {} entries, {} opening with a value",
        full.entries, full.opening_with_a_value, some.entries, some.opening_with_a_value
    );

    assert_eq!(full.entries, full.metrics);
    assert_eq!(
        full.opening_with_a_value, full.entries,
        "trial1 no longer answers every quantity, so it is no longer the half of this pair that \
         passes either build"
    );

    assert_eq!(some.entries, some.metrics);
    assert!(
        some.opening_with_a_value < some.entries,
        "trial2 answers every quantity now, so this pair can no longer tell a build that drops \
         valueless entries from one that keeps them"
    );
    assert_eq!(
        some.entries - some.opening_with_a_value,
        2,
        "trial2 no longer leaves two quantities without a number"
    );
    // Both of those two say why, so the entries this pair is about are accounts rather than
    // blanks, and the count of non-empty ones is what separates the two readings of eleven.
    assert_eq!(some.non_empty, some.entries);
}

/// One recording, two requests naming the same rule two ways, one set of keys.
///
/// A caller who names a rule and a caller who lets the spine run it under its default are
/// asking for one number. A result whose columns differ by which of the two was written is the
/// engine answering a question about the request rather than about the recording, and the
/// reader who filtered for the column that vanished cannot tell it from a column nobody asked
/// for.
///
/// Measured on `subject01_trial2`, where the rule declines because the recording stops while
/// the athlete is still in the air: naming no rule reported 11 quantities with
/// `flight_time_seconds` among them, and naming the rule reported 10 without it.
///
/// The comparison is two key sets against each other, so there is no count written here to
/// drift and no denominator to get wrong. The control is the recording that answers, where both
/// requests agree already, so an implementation that always agreed with itself passes the
/// control and reddens on the recording where the rule declines.
#[test]
fn naming_a_rule_and_letting_the_spine_run_it_ask_for_the_same_quantities() {
    let named = binding("flight_time", "flight_time.takeoff_to_touchdown");

    for (recording, answers) in [("subject01_trial2", false), ("subject01_trial1", true)] {
        let trial = trial_at(&format!("{FIXTURES}{recording}.force.txt"));
        let by_the_spine = run(&trial, &spine_request()).expect("the request is well formed");
        let by_name = run(&trial, &named).expect("the request is well formed");

        let spine_keys: BTreeSet<&str> = by_the_spine
            .metrics
            .iter()
            .map(|metric| metric.key.as_str())
            .collect();
        let named_keys: BTreeSet<&str> = by_name
            .metrics
            .iter()
            .map(|metric| metric.key.as_str())
            .collect();
        println!(
            "{recording}: {} quantities naming no rule, {} naming the rule",
            spine_keys.len(),
            named_keys.len()
        );
        assert_eq!(
            spine_keys, named_keys,
            "{recording} answers a different set of quantities depending on which way the rule \
             was named"
        );

        // The two halves of the pair are not the same case, and the flag says which is which:
        // on the recording that lands the rule answers under both requests, so it cannot tell a
        // build that keeps a declining rule's column from one that drops it.
        let flight_time = by_name
            .metric("flight_time_seconds")
            .expect("the quantity the requests differ over");
        assert_eq!(
            flight_time.value.is_some(),
            answers,
            "{recording} no longer {} the flight time, so this half of the pair is not the case \
             it was chosen for",
            if answers { "answers" } else { "declines" }
        );
    }
}
