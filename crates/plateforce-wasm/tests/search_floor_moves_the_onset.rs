//! The search floor has to move the onset it bounds.
//!
//! `onset.op.search_floor` is where a rule is told not to look before some time, and the
//! registry marks the operator as the thing that sets a rule's failure rate rather than its
//! threshold. An operator that appears in the record and changes no number is worse than a
//! missing one: the fingerprint names a value that did not produce the answer.

use std::collections::BTreeMap;

use plateforce_analysis::{run, AnalysisRequest, MethodChoice, WeighingChoice};
use plateforce_core::STANDARD_GRAVITY_METERS_PER_SECOND_SQUARED;
use plateforce_wasm::demo::synthetic_countermovement_jump;

const SAMPLE_RATE_HZ: f64 = 1200.0;

fn request_with(onset_parameters: &[(&str, f64)]) -> AnalysisRequest {
    AnalysisRequest {
        weighing: WeighingChoice {
            method_id: "bwepoch.fixed_window".into(),
            start_index: None,
            parameters: BTreeMap::from([("duration".to_string(), 1.0)]),
            options: BTreeMap::new(),
            ..Default::default()
        },
        onset: MethodChoice {
            method_id: "onset.threshold.noise_relative".into(),
            parameters: onset_parameters
                .iter()
                .map(|(name, value)| (name.to_string(), *value))
                .collect(),
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

fn onset_seconds(onset_parameters: &[(&str, f64)]) -> f64 {
    let trial = synthetic_countermovement_jump();
    let response = run(&trial, &request_with(onset_parameters)).expect("the rule ran");
    let index = response.onset_index.expect("the rule placed an onset");
    index as f64 / SAMPLE_RATE_HZ
}

/// The synthetic trial moves at 3.44 s. A floor set after that has to push the answer past
/// itself, because the samples carrying the real departure are no longer in the range.
#[test]
fn a_floor_after_the_movement_pushes_the_onset_past_it() {
    let unfloored = onset_seconds(&[]);
    let floored = onset_seconds(&[("floor_seconds", 3.5)]);

    assert!(
        (unfloored - 3.44).abs() < 0.05,
        "the unfloored rule found {unfloored:.4} s, and the trial moves at 3.44 s"
    );
    assert!(
        floored > unfloored + 0.02,
        "a floor at 3.5 s left the onset at {floored:.4} s against {unfloored:.4} s unfloored, \
         so the operator named in the record did not produce the answer"
    );
}

/// The record has to carry the value the caller stated, against the operator that read it,
/// and not report it as a value the rule chose for itself.
#[test]
fn the_stated_floor_is_recorded_against_its_own_operator() {
    let trial = synthetic_countermovement_jump();
    let response = run(&trial, &request_with(&[("floor_seconds", 3.5)])).expect("the rule ran");

    let operator = response
        .bound_methods
        .iter()
        .find(|bound| bound.method_id == "onset.op.search_floor")
        .expect("the search floor is recorded as the registry entry it is");

    let (_, shown) = operator
        .bound_parameters
        .iter()
        .find(|(name, _)| name == "floor_seconds")
        .expect("the operator carries the value it read");

    assert_eq!(shown, "3.5");
    assert!(
        !operator
            .assumed_parameters()
            .contains(&"floor_seconds".to_string()),
        "a value the caller stated came back marked as one the rule assumed"
    );
}
