//! A chain names the rules that placed the landmarks, every operator entry those rules bound,
//! and the entries a number rests on that placed no landmark at all.
//!
//! Two families are easy for a chain to lose. The five onset and two takeoff operator entries,
//! which a chain written from the request's word rather than from what the rules handed back
//! never sees, though which crossing each operator selected moves the sample its rule placed.
//! And the four integration entries, which place no sample, so the record of what a rule read
//! cannot reach them, while two of them give different velocities from one recording.

use std::collections::BTreeMap;

use plateforce_analysis::{run, AnalysisRequest, AnalysisResponse, MethodChoice, WeighingChoice};
use plateforce_core::Trial;

const INTEGRATION_IDS: [&str; 4] = [
    "integration.rule.trapezoid",
    "integration.direction.forward",
    "integration.start.detected_onset",
    "integration.anchor.single_point",
];

const NET_IMPULSE_CONSTRUCT: &str = "net_impulse";
const NET_IMPULSE_ID: &str = "impulse.net_vertical.as_performance_determinant";

/// Stance that drifts, a countermovement, a push, flight, then landing.
///
/// The stance drifts because a perfectly flat one has no spread, and an onset rule whose
/// threshold is a multiple of that spread declines on a band of zero width, so the recording
/// would carry none of the numbers this reads.
fn a_countermovement_jump() -> Trial {
    let mut force: Vec<f64> = (0..1200).map(|index| 600.0 + index as f64 * 0.01).collect();
    force.extend((0..360).map(|index| 612.0 - 312.0 * (index as f64 / 360.0)));
    force.extend((0..360).map(|index| 300.0 + 1200.0 * (index as f64 / 360.0)));
    force.extend(std::iter::repeat_n(0.0, 600));
    force.extend(std::iter::repeat_n(1800.0, 240));
    force.extend(std::iter::repeat_n(612.0, 600));
    Trial::new(force, 1200.0).unwrap()
}

fn request() -> AnalysisRequest {
    AnalysisRequest {
        weighing: WeighingChoice {
            method_id: "bwepoch.fixed_window".into(),
            parameters: BTreeMap::from([("duration".to_string(), 0.8)]),
            ..Default::default()
        },
        onset: MethodChoice {
            method_id: "onset.threshold.noise_relative".into(),
            ..Default::default()
        },
        takeoff: MethodChoice {
            method_id: "takeoff.threshold.absolute_force".into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

/// The same analysis with the caller naming the rule for itself, which is the arrival the
/// spine's own run has to agree with.
fn request_naming_the_rule() -> AnalysisRequest {
    let mut named = request();
    named.derived.insert(
        NET_IMPULSE_CONSTRUCT.to_string(),
        MethodChoice {
            method_id: NET_IMPULSE_ID.to_string(),
            ..Default::default()
        },
    );
    named
}

fn chain(response: &AnalysisResponse, key: &str) -> Vec<String> {
    response
        .metric(key)
        .unwrap_or_else(|| panic!("no {key} in the result"))
        .contributing_method_ids
        .clone()
}

/// The operator entries the landmark rules bound on this recording, read off the result rather
/// than written down here. A list written down here would pass while the rules bound nothing.
fn operators_that_ran(response: &AnalysisResponse) -> Vec<String> {
    response
        .bound_methods
        .iter()
        .map(|bound| bound.method_id.clone())
        .filter(|id| id.starts_with("onset.op.") || id.starts_with("takeoff.op."))
        .collect()
}

/// Both families, on a number that reached the caller through the phase that runs a rule the
/// request named.
#[test]
fn a_number_the_derived_phase_produced_names_the_operators_and_the_integration_entries() {
    let response = run(&a_countermovement_jump(), &request_naming_the_rule()).expect("it runs");

    // A population this guard would otherwise pass by having nothing to read, and both
    // families rather than a count: a total alone is met by one family twice over.
    let operators = operators_that_ran(&response);
    assert!(
        operators.iter().any(|id| id.starts_with("onset.op.")),
        "no onset operator was bound: {operators:?}"
    );
    assert!(
        operators.iter().any(|id| id.starts_with("takeoff.op.")),
        "no takeoff operator was bound: {operators:?}"
    );
    println!("operator entries bound on this recording: {operators:?}");
    assert!(
        operators.len() >= 6,
        "only {} operator entries were bound, so this recording cannot show the chain carrying \
         them: {operators:?}",
        operators.len()
    );

    let velocity = chain(&response, "takeoff_velocity_meters_per_second");
    for operator in &operators {
        assert!(
            velocity.contains(operator),
            "the takeoff velocity did not name {operator}, which its landmark rule bound: \
             {velocity:?}"
        );
    }
    for id in INTEGRATION_IDS {
        assert!(
            velocity.contains(&id.to_string()),
            "the takeoff velocity is read off the integrated series and did not name {id}: \
             {velocity:?}"
        );
    }

    // The other half, and the reason the entries are declared per number rather than per rule.
    // One entry describes both of these, and the impulse is integrated over the interval
    // directly rather than read off the series those four built.
    let impulse = chain(&response, "net_impulse_newton_seconds");
    for operator in &operators {
        assert!(
            impulse.contains(operator),
            "the net impulse did not name {operator}: {impulse:?}"
        );
    }
    for id in INTEGRATION_IDS {
        assert!(
            !impulse.contains(&id.to_string()),
            "the net impulse is integrated directly and named {id}: {impulse:?}"
        );
    }
}

/// One entry, one record, whichever way the caller arrived at it.
///
/// The two chains are compared against each other last and never alone: a family vanishing from
/// both would leave them equal. Each is first checked against the operators the rules bound
/// and against the four the series was integrated under.
#[test]
fn one_entry_leaves_one_chain_whether_or_not_the_caller_named_the_rule() {
    let trial = a_countermovement_jump();
    let spine = run(&trial, &request()).expect("the spine runs");
    let named = run(&trial, &request_naming_the_rule()).expect("the named request runs");

    for key in [
        "takeoff_velocity_meters_per_second",
        "net_impulse_newton_seconds",
    ] {
        let arrived_by_spine = chain(&spine, key);
        let arrived_by_name = chain(&named, key);
        for operator in operators_that_ran(&spine) {
            assert!(
                arrived_by_spine.contains(&operator),
                "{key} reached through the spine without naming {operator}: {arrived_by_spine:?}"
            );
        }
        assert_eq!(
            arrived_by_spine, arrived_by_name,
            "{key} carries a different chain depending only on whether the caller named the rule"
        );
        assert_eq!(
            spine.metric(key).and_then(|metric| metric.value),
            named.metric(key).and_then(|metric| metric.value),
            "{key} carries a different value depending only on whether the caller named the rule"
        );
    }
}

/// The entries the three headline numbers name are entries this build runs, which is the whole
/// of what a reader needs to check the arithmetic against the rule the number cites.
#[test]
fn the_entry_a_headline_number_names_is_one_that_ran() {
    let response = run(&a_countermovement_jump(), &request()).expect("the spine runs");
    let ran: Vec<String> = response
        .bound_methods
        .iter()
        .map(|bound| bound.method_id.clone())
        .collect();

    let mut checked = 0usize;
    for key in [
        "takeoff_velocity_meters_per_second",
        "net_impulse_newton_seconds",
        "reactive_strength_index_modified",
        "jump_height_from_flight_time_meters",
        "jump_height_from_takeoff_meters",
    ] {
        let named = response
            .metric(key)
            .unwrap_or_else(|| panic!("no {key} in the result"))
            .computed_by
            .clone()
            .unwrap_or_else(|| panic!("{key} names no entry for its arithmetic"));
        assert!(
            ran.contains(&named),
            "{key} is labelled {named}, which did not run on this analysis: {ran:?}"
        );
        checked += 1;
    }
    println!("{checked} headline numbers name an entry that ran");
    assert_eq!(checked, 5);
}

/// The arithmetic behind a routed number lives in the rule the number names and nowhere else.
///
/// Read off the source, because no comparison of results can see it. A second expression of one
/// of these in the spine calls the same core function as the rule does, so it agrees with the
/// rule on every recording and the two chains stay identical, and every guard above stays green
/// while it is free to stop agreeing at the first edit to either.
#[test]
fn the_spine_holds_no_second_expression_of_a_number_a_rule_now_produces() {
    let source = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/pipeline.rs"))
        .expect("the spine is readable");

    // A control. A scan that read nothing reports every arithmetic below as absent, and reads
    // exactly like a scan that read the file and found none of them.
    assert!(
        source.contains("run_spine_default"),
        "the scan read no source, so its verdict means nothing"
    );

    let mut checked = 0usize;
    for (arithmetic, owned_by) in [
        ("integrate_offset_newton_seconds", NET_IMPULSE_ID),
        ("takeoff_velocity_meters_per_second(", NET_IMPULSE_ID),
        (
            "jump_height_from_takeoff_velocity(",
            "jumpheight.takeoff.impulse_momentum",
        ),
        (
            "reactive_strength_index_modified(",
            "rsimod.jh_tov_over_ttt",
        ),
    ] {
        assert!(
            !source.contains(arithmetic),
            "the spine calls {arithmetic}, which is the arithmetic {owned_by} describes and runs"
        );
        checked += 1;
    }
    println!("{checked} routed quantities have one expression, in the rule that names them");
    assert_eq!(checked, 4);
}
