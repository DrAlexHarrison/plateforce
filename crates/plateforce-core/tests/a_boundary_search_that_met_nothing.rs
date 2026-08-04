//! What the phase models do when a search they compose returns an instant without meeting
//! the condition the model names there.
//!
//! The split model's fourth boundary is the instant centre of mass velocity turns positive,
//! searched from peak negative velocity to takeoff. Where no such instant exists the search
//! returns the sample after the velocity minimum, reproducing the tool the rule is drawn from.
//! Published as a boundary that is an eccentric braking phase one sample long.
//!
//! Measured on subject 01 trial 1 with the landmarks held at the values the honest run placed,
//! so system weight is the only thing that varies between the rows.
//!
//! Run it with
//! `cargo test -p plateforce-core --test a_boundary_search_that_met_nothing -- --nocapture`.

use plateforce_core::phases::{
    braking_start_by_force_return, center_of_mass_acceleration, cumulative_trapezoid,
    force_reference_crossing, phase_model_unloading_yielding_split, phase_model_unweighting_single,
    propulsion_end_by_force_crossing, CrossingDirection, PhaseModelOutcome,
};
use plateforce_core::series::{
    centre_of_mass_velocity_meters_per_second, IntegrationAnchor, IntegrationDirection,
    IntegrationSpec, IntegrationStart, QuadratureRule, VelocitySeries,
};
use plateforce_core::trial::{CentralTendency, WeighingEpoch};
use plateforce_core::{DispersionEstimator, Trial};

const SAMPLE_RATE_HZ: f64 = 1200.0;
const GRAVITY_METERS_PER_SECOND_SQUARED: f64 = 9.806_65;
const WEIGHING_WINDOW_SECONDS: f64 = 0.2;
const UNLOADING_DROP_PERCENT: f64 = 2.5;

/// The landmarks `onset.threshold.noise_relative` at k = 5 and
/// `takeoff.threshold.absolute_force` at 20 N place on this recording.
const ONSET_INDEX: usize = 4091;
const TAKEOFF_INDEX: usize = 5014;

fn subject01_trial1() -> Trial {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../plateforce-conformance/fixtures/subject01_trial1.force.txt");
    let text = std::fs::read_to_string(&path).expect("the fixture reads");
    let force: Vec<f64> = text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            line.split('\t')
                .next()
                .expect("a first column")
                .trim()
                .parse()
                .expect("a number")
        })
        .collect();
    Trial::new(force, SAMPLE_RATE_HZ).expect("a trial")
}

/// A weighing window of the stated length starting at the stated second, which is what
/// `bwepoch.manual_placement` gives a caller who states where the window sits.
fn weighing_window_at(trial: &Trial, start_seconds: f64) -> WeighingEpoch {
    let start_index = (start_seconds * SAMPLE_RATE_HZ) as usize;
    let shifted =
        Trial::new(trial.force()[start_index..].to_vec(), SAMPLE_RATE_HZ).expect("the window fits");
    let mut epoch = WeighingEpoch::fixed_window(
        &shifted,
        WEIGHING_WINDOW_SECONDS,
        CentralTendency::Mean,
        DispersionEstimator::Sample,
    )
    .expect("the window fits");
    epoch.start_index += start_index;
    epoch.end_index += start_index;
    epoch
}

fn velocity_from(trial: &Trial, epoch: &WeighingEpoch) -> VelocitySeries {
    let spec = IntegrationSpec {
        quadrature: QuadratureRule::Trapezoid,
        direction: IntegrationDirection::Forward,
        start: IntegrationStart::DetectedOnset { index: ONSET_INDEX },
        anchor: IntegrationAnchor::SinglePoint { index: ONSET_INDEX },
    };
    centre_of_mass_velocity_meters_per_second(
        trial,
        epoch,
        &spec,
        GRAVITY_METERS_PER_SECOND_SQUARED,
    )
}

fn split_model_at(trial: &Trial, start_seconds: f64) -> (f64, PhaseModelOutcome) {
    let epoch = weighing_window_at(trial, start_seconds);
    let velocity = velocity_from(trial, &epoch);
    let peak =
        plateforce_core::peak::index_of_maximum_over(trial.force(), ONSET_INDEX, TAKEOFF_INDEX)
            .expect("a propulsive peak");
    (
        epoch.system_weight_newtons,
        phase_model_unloading_yielding_split(
            trial.force(),
            &velocity,
            epoch.system_weight_newtons,
            UNLOADING_DROP_PERCENT,
            0,
            peak,
            TAKEOFF_INDEX,
        ),
    )
}

/// A window over quiet standing weighs the athlete; three windows over the movement do not,
/// and under all three the search for a positive velocity meets nothing between the velocity
/// minimum and takeoff.
///
/// Both directions of weighing error reach it. An under-estimate leaves velocity positive
/// across the whole window so no sample sits at or below the threshold to rise from; an
/// over-estimate leaves it negative across the whole window so nothing rises above.
#[test]
fn the_split_model_refuses_the_boundary_its_velocity_search_did_not_meet() {
    let trial = subject01_trial1();

    let (honest_weight_newtons, honest) = split_model_at(&trial, 0.0);
    let PhaseModelOutcome::Placed(placed) = honest else {
        panic!("the honest window did not place the model: {honest:?}");
    };
    assert_eq!(placed.indices, vec![1167, 4299, 4490, 4719, 5014]);
    assert!(placed.indices.windows(2).all(|pair| pair[0] < pair[1]));
    println!("quiet standing weighs {honest_weight_newtons:.2} N, boundaries {placed:?}");

    // Under, then over, then further over. The velocity minimum is the anchor and the sample
    // after it is what the search returns, so the interval the model would have published as
    // its eccentric braking phase is exactly one sample wide.
    let expected = [(3.5, 4368usize), (3.8, 4677), (4.0, 4740)];
    for (start_seconds, anchor) in expected {
        let (weight_newtons, outcome) = split_model_at(&trial, start_seconds);
        let PhaseModelOutcome::BoundaryNotCrossed {
            boundary_position,
            anchor_index,
            returned_index,
        } = outcome
        else {
            panic!("a window at {start_seconds} s placed the model: {outcome:?}");
        };
        assert_eq!(boundary_position, 3);
        assert_eq!(anchor_index, anchor);
        assert_eq!(
            returned_index,
            anchor_index + 1,
            "the search returned something other than the sample after its anchor"
        );
        println!(
            "a window at {start_seconds} s weighs {weight_newtons:.2} N, and the boundary it \
             would have published spans {:.4} s to {:.4} s",
            trial.time_at(anchor_index),
            trial.time_at(returned_index)
        );
    }

    // Three of the four rows meet nothing and one does, so an assertion that fired on every
    // row or on none of them would fail here rather than pass quietly.
    assert_ne!(honest_weight_newtons, 0.0);
}

/// The two ends of the same recording, held against the same reference, disagree about
/// whether a crossing exists, which is what the flag exists to carry.
#[test]
fn a_force_crossing_that_returns_its_own_end_of_the_search_says_so() {
    let weight = 700.0;
    let mut force = vec![weight; 200];
    force.extend((0..100).map(|i| weight - 3.0 * i as f64));
    force.extend((0..100).map(|i| weight - 300.0 + 9.0 * i as f64));
    force.extend((0..100).map(|i| weight + 600.0 - 12.0 * i as f64));
    force.extend(vec![weight - 600.0; 100]);
    let peak = 400;
    let takeoff = force.len() - 1;

    let rising = force_reference_crossing(&force, weight, 0, peak, CrossingDirection::Rising)
        .expect("the search answers");
    assert!(rising.is_true_crossing);
    assert!(rising.index < peak);

    // A reference above everything the interval carries. The rise never happens, and the
    // sample the search returns is the bound it was given rather than a crossing.
    let above_everything = 5000.0;
    let truncated =
        force_reference_crossing(&force, above_everything, 0, peak, CrossingDirection::Rising)
            .expect("the search answers");
    assert!(!truncated.is_true_crossing);
    assert_eq!(truncated.index, peak);

    // The mirror. A falling search whose interval never rises above the reference returns the
    // force maximum it anchored on, which is not a fall through anything.
    let falling =
        propulsion_end_by_force_crossing(&force, weight, rising.index, takeoff).expect("answers");
    assert!(falling.is_true_crossing);
    let collapsed =
        propulsion_end_by_force_crossing(&force, above_everything, rising.index, takeoff)
            .expect("answers");
    assert!(!collapsed.is_true_crossing);
    assert_eq!(collapsed.index, collapsed.anchor_index);

    // The wrapper the conformance harness calls is the same search, so the flag reaches it
    // rather than being a property of the generalised form alone.
    let wrapped =
        braking_start_by_force_return(&force, 0, above_everything, peak).expect("answers");
    assert_eq!(wrapped, truncated);
}

/// The single-phase model's end boundary is that rising crossing, so it refuses on the same
/// recording where the crossing does not exist and places on the one where it does.
#[test]
fn the_single_unweighting_model_refuses_the_boundary_its_search_did_not_meet() {
    let weight = 700.0;
    let mut force = vec![weight; 200];
    force.extend((0..100).map(|i| weight - 3.0 * i as f64));
    force.extend((0..100).map(|i| weight - 300.0 + 9.0 * i as f64));
    force.extend((0..100).map(|i| weight + 600.0 - 12.0 * i as f64));
    force.extend(vec![weight - 600.0; 100]);
    let peak = 400;

    let placed = phase_model_unweighting_single(&force, weight, 0, peak);
    let PhaseModelOutcome::Placed(boundaries) = placed else {
        panic!("the model placed nothing on a trace that carries the crossing: {placed:?}");
    };
    assert_eq!(boundaries.indices.len(), 2);
    assert!(boundaries.indices[0] < boundaries.indices[1]);

    // A system weight above the propulsive peak. Force never returns up through it, so the
    // search reaches the peak it was bounded at and the model declines rather than publishing
    // the propulsive peak as the end of unweighting.
    let above_the_peak = force.iter().copied().fold(f64::NEG_INFINITY, f64::max) + 1.0;
    let refused = phase_model_unweighting_single(&force, above_the_peak, 0, peak);
    let PhaseModelOutcome::BoundaryNotCrossed {
        boundary_position,
        returned_index,
        ..
    } = refused
    else {
        panic!("the model published a boundary its search did not meet: {refused:?}");
    };
    assert_eq!(boundary_position, 1);
    assert_eq!(returned_index, peak);
}

/// The velocity curve the split model reads, built the way the analysis layer builds it, so a
/// reader can take the numbers in the first test again from the force column alone.
#[test]
fn the_velocity_the_model_reads_comes_from_the_weighed_system_weight() {
    let trial = subject01_trial1();
    let epoch = weighing_window_at(&trial, 3.8);
    let mass = epoch.system_mass_kilograms(GRAVITY_METERS_PER_SECOND_SQUARED);
    let acceleration =
        center_of_mass_acceleration(trial.force(), epoch.system_weight_newtons, mass);
    let integrated = cumulative_trapezoid(&acceleration, trial.sample_interval_seconds());
    let series = velocity_from(&trial, &epoch);

    // Both curves are the same shape; the series pins its zero at onset and the bare integral
    // pins its own at sample zero, so the two differ by one constant everywhere.
    let offset = series.at(ONSET_INDEX).unwrap() - integrated[ONSET_INDEX];
    for index in [4299usize, 4677, 4678, 5013] {
        let difference = series.at(index).unwrap() - integrated[index] - offset;
        assert!(difference.abs() < 1e-9, "{index}: {difference}");
    }
    // Velocity is below zero across the whole interval the search covers, which is why the
    // search meets nothing there.
    for index in 4677..TAKEOFF_INDEX {
        assert!(
            series.at(index).unwrap() < 0.0,
            "velocity turned positive at {index}, so this trace cannot show what is claimed"
        );
    }
}
