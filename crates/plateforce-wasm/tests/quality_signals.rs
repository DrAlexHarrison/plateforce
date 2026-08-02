//! The confident wrong number this build offers, reproduced, and the signal that catches it.
//!
//! `onset.threshold.last_within_band` is one of five rules in the onset picker on the
//! demonstration trial. It places the start of the jump after the unweighting dip, so the
//! net impulse is over-counted and the jump height it produces is 73.9 cm on a trace whose
//! flight time says 42.4 cm. Both numbers render in the same panel with nothing to tell
//! them apart, which is the field's defining failure reproduced inside the tool built to
//! document it.
//!
//! The test reproduces both numbers rather than asserting that the remedy fires. A test
//! that only asserted the fix cannot tell a fix from a fixture that never had the defect.
//! It pins the disagreements and the onset offset in samples rather than the four heights:
//! a legitimate change to the demonstration trace would fail four float pins with no
//! diagnosis, and fails one disagreement with a name.

use std::collections::BTreeMap;

use plateforce_analysis::quality::{signals, QualityStatus};
use plateforce_analysis::{
    run, AnalysisRequest, AnalysisResponse, MethodChoice, WeighingChoice,
};
use plateforce_core::{Trial, STANDARD_GRAVITY_METERS_PER_SECOND_SQUARED};
use plateforce_wasm::demo::synthetic_countermovement_jump;

const FROM_TAKEOFF: &str = "jump_height_from_takeoff_meters";
const FROM_FLIGHT: &str = "jump_height_from_flight_time_meters";

fn request_with_onset(method_id: &str) -> AnalysisRequest {
    AnalysisRequest {
        weighing: WeighingChoice {
            method_id: "bwepoch.fixed_window".into(),
            start_index: None,
            parameters: BTreeMap::from([("duration".to_string(), 1.0)]),
            options: BTreeMap::new(),
        },
        onset: MethodChoice {
            method_id: method_id.into(),
            ..Default::default()
        },
        takeoff: MethodChoice {
            method_id: "takeoff.threshold.absolute_force".into(),
            parameters: BTreeMap::from([("threshold_n".to_string(), 20.0)]),
            ..Default::default()
        },
        touchdown_index: None,
        gravity_meters_per_second_squared: STANDARD_GRAVITY_METERS_PER_SECOND_SQUARED,
        registry_backed_ids: Vec::new(),
    }
}

fn metric(response: &AnalysisResponse, key: &str) -> f64 {
    response
        .metrics
        .iter()
        .find(|entry| entry.key == key)
        .and_then(|entry| entry.value)
        .unwrap_or_else(|| panic!("the demonstration trial produced no {key}"))
}

fn analyse(trial: &Trial, method_id: &str) -> AnalysisResponse {
    run(trial, &request_with_onset(method_id)).expect("the demonstration trial analyses")
}

#[test]
fn two_onset_rules_produce_two_jump_heights_and_only_one_is_flagged() {
    let trial = synthetic_countermovement_jump();

    let clean = analyse(&trial, "onset.threshold.noise_relative");
    let broken = analyse(&trial, "onset.threshold.last_within_band");

    let clean_takeoff = metric(&clean, FROM_TAKEOFF);
    let clean_flight = metric(&clean, FROM_FLIGHT);
    let broken_takeoff = metric(&broken, FROM_TAKEOFF);
    let broken_flight = metric(&broken, FROM_FLIGHT);

    println!("onset.threshold.noise_relative     impulse {clean_takeoff} m, flight {clean_flight} m, onset sample {:?}", clean.onset_index);
    println!("onset.threshold.last_within_band   impulse {broken_takeoff} m, flight {broken_flight} m, onset sample {:?}", broken.onset_index);
    println!(
        "warnings: {} and {}, refusals: {} and {}",
        clean.warnings.len(),
        broken.warnings.len(),
        clean.refusals.len(),
        broken.refusals.len()
    );

    // Neither run says anything is wrong, which is the whole of the defect.
    assert!(clean.warnings.is_empty() && broken.warnings.is_empty());
    assert!(clean.refusals.is_empty() && broken.refusals.is_empty());

    let clean_disagreement = (clean_takeoff - clean_flight).abs();
    let broken_disagreement = (broken_takeoff - broken_flight).abs();
    println!(
        "disagreement: {:.4} cm under the clean rule, {:.4} cm under the broken one",
        100.0 * clean_disagreement,
        100.0 * broken_disagreement
    );
    assert!(
        (clean_disagreement - 0.0114).abs() < 0.001,
        "the two routes agree to 1.1 cm under noise_relative, read {clean_disagreement}"
    );
    assert!(
        (broken_disagreement - 0.3147).abs() < 0.001,
        "the two routes sit 31.5 cm apart under last_within_band, read {broken_disagreement}"
    );

    let offset = broken.onset_index.unwrap() - clean.onset_index.unwrap();
    assert_eq!(offset, 354, "the broken rule places the start 354 samples later");

    assert!(
        signals(&clean).is_empty(),
        "a trial whose two routes agree carries no signal"
    );
    let fired = signals(&broken);
    assert_eq!(fired.len(), 1, "the broken rule raises exactly one signal");
    assert_eq!(fired[0].status, QualityStatus::Disagrees);
    assert!(!fired[0].remedy.is_empty());
    assert_eq!(fired[0].remedy_construct, "movement_onset");
    assert!(fired[0].qualifies.contains(&FROM_TAKEOFF));
    assert!(fired[0].qualifies.contains(&FROM_FLIGHT));
    println!("signal: {} | {}", fired[0].label, fired[0].remedy);
}

#[test]
fn a_trace_that_never_lands_says_the_check_could_not_run() {
    // Truncating at takeoff is the shape of the 211 of 244 corpus trials whose recording
    // ends at the plate's floor. Silence there reads exactly like a check that ran.
    let full = synthetic_countermovement_jump();
    let response = analyse(&full, "onset.threshold.noise_relative");
    let takeoff = response.takeoff_index.expect("the demonstration trial takes off");
    let truncated = Trial::new(full.force()[..takeoff + 20].to_vec(), full.sample_rate_hz())
        .expect("a trace cut just after takeoff is still a trial");

    let cut = analyse(&truncated, "onset.threshold.noise_relative");
    let fired = signals(&cut);
    println!(
        "truncated at sample {}: {} signals, flight-time height {:?}",
        takeoff + 20,
        fired.len(),
        cut.metrics.iter().find(|m| m.key == FROM_FLIGHT).and_then(|m| m.value)
    );
    assert_eq!(fired.len(), 1);
    assert_eq!(fired[0].status, QualityStatus::Incomparable);
    assert!(fired[0].value.is_none());
    assert!(!fired[0].remedy.is_empty());
}
