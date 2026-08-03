//! A rule that ran and found nothing publishes the code for that, on every surface.
//!
//! The sentence a rule writes goes to somebody reading the trace. The code goes to a program
//! branching on it, and a program cannot recover from a code that names the wrong condition.
//! This drives a real rule to a real dead end and reads the record back.

use plateforce_analysis::document::refusal_from_rule;
use plateforce_analysis::{run, AnalysisRequest, MethodChoice, WeighingChoice};
use plateforce_core::{RefusalCode, Trial, STANDARD_GRAVITY_METERS_PER_SECOND_SQUARED};
use std::collections::BTreeMap;

/// A standing epoch, a countermovement, a flight and a landing, at a rate the corpus uses.
fn a_jump() -> Trial {
    let sample_rate_hz = 1200.0_f64;
    let standing_newtons = 700.0_f64;
    let samples = |seconds: f64| (seconds * sample_rate_hz) as usize;
    let mut force: Vec<f64> = Vec::new();
    force.extend(std::iter::repeat_n(standing_newtons, samples(1.5)));
    for index in 0..samples(0.30) {
        force.push(standing_newtons * (1.0 - 0.45 * (index as f64 / samples(0.30) as f64)));
    }
    for index in 0..samples(0.30) {
        force.push(standing_newtons * (0.55 + 1.45 * (index as f64 / samples(0.30) as f64)));
    }
    force.extend(std::iter::repeat_n(2.0, samples(0.40)));
    for index in 0..samples(0.10) {
        force.push(standing_newtons * (0.01 + 2.5 * (index as f64 / samples(0.10) as f64)));
    }
    force.extend(std::iter::repeat_n(standing_newtons, samples(1.0)));
    Trial::new(force, sample_rate_hz).expect("the shaped trace is a trial")
}

fn request_asking_for(threshold_newtons: f64) -> AnalysisRequest {
    AnalysisRequest {
        weighing: WeighingChoice {
            method_id: "bwepoch.fixed_window".into(),
            parameters: BTreeMap::from([("duration".to_string(), 1.0)]),
            ..Default::default()
        },
        onset: MethodChoice {
            method_id: "onset.threshold.absolute_force".into(),
            parameters: BTreeMap::from([("threshold_n".to_string(), threshold_newtons)]),
            ..Default::default()
        },
        takeoff: MethodChoice {
            method_id: "takeoff.threshold.absolute_force".into(),
            ..Default::default()
        },
        touchdown_index: None,
        gravity_meters_per_second_squared: STANDARD_GRAVITY_METERS_PER_SECOND_SQUARED,
        registry_backed_ids: Vec::new(),
        ..Default::default()
    }
}

#[test]
fn a_threshold_nothing_reaches_is_a_no_crossing_rather_than_an_unknown_parameter() {
    let trial = a_jump();

    // The control. The same rule, a threshold the trace does reach, and a landmark comes back.
    let found = run(&trial, &request_asking_for(20.0)).expect("the rule ran");
    assert!(
        found.onset_index.is_some(),
        "the control found no onset either, so the assertion below is about the trace"
    );

    let declined = run(&trial, &request_asking_for(99_999.0)).expect("the rule ran");
    let refusal = declined
        .refusals
        .iter()
        .find(|rule| rule.construct == "movement_onset")
        .map(refusal_from_rule)
        .expect("the onset rule declined");

    assert_eq!(refusal.code, RefusalCode::NoCrossing);
    // `threshold_n` is the parameter, not a sentence about it. A caller reading this to say
    // which value to change gets a value, and a caller matching on the code gets the condition.
    assert_eq!(refusal.parameter.as_deref(), Some("threshold_n"));
    assert_eq!(refusal.value, Some(99_999.0));
    assert_eq!(refusal.method_id, "onset.threshold.absolute_force");
    assert!(
        refusal.detail.contains_key("search_bound_seconds"),
        "the bound the rule searched is a number the record carries, and it says {:?}",
        refusal.detail
    );
}
