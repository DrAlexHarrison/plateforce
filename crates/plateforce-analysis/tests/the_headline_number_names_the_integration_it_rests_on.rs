//! Jump height from takeoff velocity is read off an integrated series, and the four choices
//! behind that series belong in its chain.
//!
//! `integration.start.trial_start` is deprecated and `integration.start.detected_onset` is
//! recommended, both entries force a decision, and they give different velocities from one
//! recording. A result that reports the velocity without naming them reports three of the
//! rules behind the number and hides four.

use std::collections::BTreeMap;

use plateforce_analysis::{run, AnalysisRequest, AnalysisResponse, MethodChoice, WeighingChoice};
use plateforce_core::series::{
    centre_of_mass_velocity_meters_per_second, IntegrationAnchor, IntegrationDirection,
    IntegrationSpec, IntegrationStart, QuadratureRule,
};
use plateforce_core::{takeoff_velocity_integration_spec, Landmarks, Trial};

const INTEGRATION_IDS: [&str; 4] = [
    "integration.rule.trapezoid",
    "integration.direction.forward",
    "integration.start.detected_onset",
    "integration.anchor.single_point",
];

/// A jump preceded by stance that drifts, which is what a settling athlete produces and what
/// makes the two integration starts disagree.
fn a_jump_after_drifting_stance() -> Trial {
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

fn chain(response: &AnalysisResponse, key: &str) -> Vec<String> {
    response
        .metrics
        .iter()
        .find(|metric| metric.key == key)
        .unwrap_or_else(|| panic!("no {key} in the result"))
        .contributing_method_ids
        .clone()
}

/// Every number read off the integrated series names the four entries it rests on, and the
/// one integrated directly does not. Both halves: a change that sprayed the ids across every
/// metric would satisfy the first and fail the second.
#[test]
fn the_numbers_read_off_the_velocity_series_name_the_integration_and_the_impulse_does_not() {
    let response = run(&a_jump_after_drifting_stance(), &request()).expect("the spine runs");

    for key in [
        "takeoff_velocity_meters_per_second",
        "jump_height_from_takeoff_meters",
        "reactive_strength_index_modified",
    ] {
        let named = chain(&response, key);
        for id in INTEGRATION_IDS {
            assert!(
                named.contains(&id.to_string()),
                "{key} did not name {id}: {named:?}"
            );
        }
    }

    let impulse = chain(&response, "net_impulse_newton_seconds");
    for id in INTEGRATION_IDS {
        assert!(
            !impulse.contains(&id.to_string()),
            "net impulse is integrated directly and named {id}: {impulse:?}"
        );
    }
}

/// Why the chain has to carry them. The two published starts are not a formality: on a
/// recording carrying stance ahead of the movement they give different takeoff velocities,
/// and therefore different jump heights, from one trace.
#[test]
fn the_two_integration_starts_give_two_takeoff_velocities_from_one_recording() {
    let trial = a_jump_after_drifting_stance();
    let response = run(&trial, &request()).expect("the spine runs");
    let (Some(onset), Some(takeoff)) = (response.onset_index, response.takeoff_index) else {
        panic!("the spine placed no landmarks on this trace");
    };
    let landmarks = Landmarks {
        onset_index: onset,
        takeoff_index: takeoff,
        touchdown_index: trial.len() - 1,
    };
    let epoch = plateforce_core::WeighingEpoch {
        start_index: response.weighing_start_index,
        end_index: response.weighing_end_index,
        system_weight_newtons: response.levels.system_weight_newtons,
        standard_deviation_newtons: response.levels.weighing_standard_deviation_newtons,
        tied_window_count: response.weighing_epoch_tied_window_count,
        tied_weight_low_newtons: response.levels.system_weight_newtons,
        tied_weight_high_newtons: response.levels.system_weight_newtons,
    };

    let read_at_takeoff = |spec: IntegrationSpec| {
        centre_of_mass_velocity_meters_per_second(&trial, &epoch, &spec, 9.80665)
            .at(takeoff.saturating_sub(1))
            .expect("the series covers takeoff")
    };

    let recommended = read_at_takeoff(takeoff_velocity_integration_spec(&landmarks));
    let deprecated = read_at_takeoff(IntegrationSpec {
        quadrature: QuadratureRule::Trapezoid,
        direction: IntegrationDirection::Forward,
        start: IntegrationStart::TrialStart,
        anchor: IntegrationAnchor::SinglePoint { index: 0 },
    });

    println!("from detected onset {recommended:.4} m/s, from trial start {deprecated:.4} m/s");
    assert!(
        (recommended - deprecated).abs() > 0.01,
        "the two starts agreed to within 0.01 m/s at {recommended:.4} and {deprecated:.4}, so \
         this trace cannot show that the choice moves the number"
    );
}
