//! A landmark a hand placed fingerprints apart from a detection, and apart from a hand
//! placement somewhere else.
//!
//! The fingerprint answers one question: did two labs compute this number the same way. A
//! reader who drags the onset marker has supplied a sample no rule produced, so the two runs
//! did not compute it the same way however identical the rules and their values read.
//!
//! Two collisions, both live before this. A dragged onset sheds the values its rule would have
//! read, so the record thins rather than changing, and nothing in what is left says a hand
//! placed anything. A dragged takeoff does not even thin, because the takeoff rule runs under a
//! dragged marker to resolve the threshold touchdown is found against, so its record was the
//! detection's record exactly. And neither carried the sample, so two hands placing two
//! different takeoffs reached one digest over flight times of 0.000 s and 0.659 s: one of those
//! two says the athlete never left the plate.
//!
//! The sample and not a flag, which is the rule `a_landing_the_caller_placed_says_so` already
//! states for a landing the caller supplied. A flag separates a detection from a hand placement
//! and leaves two hand placements declaring they are one result.

use std::collections::BTreeMap;

use plateforce_analysis::chain::chain_of;
use plateforce_analysis::{run, AnalysisRequest, AnalysisResponse, MethodChoice, WeighingChoice};
use plateforce_core::provenance::RegistryStamp;
use plateforce_core::reporting::fingerprint;

mod common;
use plateforce_core::{Acquisition, Trial};

const SAMPLE_RATE_HZ: f64 = 1200.0;
const ONSET_RULE: &str = "onset.threshold.noise_relative";
const TAKEOFF_RULE: &str = "takeoff.threshold.absolute_force";
const INTERVAL: &str = "time_to_takeoff_seconds";
const FLIGHT: &str = "flight_time_seconds";

/// A countermovement jump that leaves the plate and lands back on it, so a number bounded by
/// takeoff and a number bounded by onset are both reported and can be read apart.
fn a_jump_that_lands() -> Trial {
    let mut force = vec![600.0; 1200];
    for (index, sample) in force.iter_mut().enumerate() {
        *sample += ((index % 17) as f64 - 8.0) * 0.4;
    }
    force.extend((0..240).map(|index| 600.0 - 220.0 * (index as f64 / 240.0)));
    force.extend((0..240).map(|index| 380.0 + 220.0 * (index as f64 / 240.0)));
    force.extend((0..660).map(|index| 600.0 + 900.0 * (index as f64 / 660.0)));
    force.extend(std::iter::repeat_n(0.0, 811));
    force.extend(std::iter::repeat_n(2400.0, 240));
    force.extend(std::iter::repeat_n(600.0, 600));
    Trial::new(force, SAMPLE_RATE_HZ).unwrap()
}

fn request(onset_at: Option<usize>, takeoff_at: Option<usize>) -> AnalysisRequest {
    common::prepared(AnalysisRequest {
        weighing: WeighingChoice {
            method_id: "bwepoch.fixed_window".into(),
            parameters: BTreeMap::from([("duration".to_string(), 0.8)]),
            ..Default::default()
        },
        onset: MethodChoice {
            method_id: ONSET_RULE.into(),
            manual_index: onset_at,
            ..Default::default()
        },
        takeoff: MethodChoice {
            method_id: TAKEOFF_RULE.into(),
            manual_index: takeoff_at,
            ..Default::default()
        },
        ..Default::default()
    })
}

fn stamp() -> RegistryStamp {
    RegistryStamp {
        version: Some("fixture-pin".to_string()),
        declared_version: Some("fixture-declares".to_string()),
        digest: Some("content-fixture".to_string()),
    }
}

/// A plate whose settings were all recorded. An unfilled block publishes no digest at all and
/// matches nothing including itself, so a comparison over two of them proves nothing either way.
fn a_recorded_plate() -> Acquisition {
    Acquisition {
        filter_at_capture: Some("none".to_string()),
        tare_state: Some("tared_before_trial".to_string()),
        plate_natural_frequency_hz: Some(400.0),
        floor_surface: Some("concrete".to_string()),
        firmware_version: Some("2.4.1".to_string()),
    }
}

fn analysed(request: AnalysisRequest) -> AnalysisResponse {
    run(&a_jump_that_lands(), &request).expect("the trace supports an analysis")
}

fn value(response: &AnalysisResponse, key: &str) -> f64 {
    response
        .metrics
        .iter()
        .find(|metric| metric.key == key)
        .and_then(|metric| metric.value)
        .unwrap_or_else(|| panic!("{key} carries no number, so there is nothing to compare"))
}

fn digest(response: &AnalysisResponse, key: &str) -> String {
    let metric = response
        .metrics
        .iter()
        .find(|metric| metric.key == key)
        .unwrap_or_else(|| panic!("{key} is absent, so there is no chain to fingerprint"));
    let chain = chain_of(response, metric, &stamp(), true);
    fingerprint(&chain.provenance, &a_recorded_plate(), SAMPLE_RATE_HZ)
        .published()
        .expect("the acquisition block is filled, so this digest publishes")
        .to_string()
}

/// Every rule on the run whose record says a hand placed its landmark, with the sample.
fn rows_a_hand_placed(response: &AnalysisResponse) -> Vec<(String, usize)> {
    response
        .bound_methods
        .iter()
        .filter_map(|bound| {
            bound
                .placed_by_hand_at_sample
                .map(|sample| (bound.method_id.clone(), sample))
        })
        .collect()
}

/// Two hands placing two different samples give two numbers, so they are two results.
///
/// Both members of each pair are hand placements. Written against a detection instead, this
/// would pass on the record thinning, which is what separated them before and is a side effect
/// rather than a rule.
#[test]
fn two_hands_placing_two_samples_do_not_fingerprint_as_one_result() {
    for (slot, key, one, other) in [
        (
            "onset",
            INTERVAL,
            request(Some(1180), None),
            request(Some(1120), None),
        ),
        (
            "takeoff",
            FLIGHT,
            request(None, Some(2300)),
            request(None, Some(2360)),
        ),
    ] {
        let (left, right) = (analysed(one), analysed(other));

        // The premise of the comparison. Two placements that gave one number would fingerprint
        // alike correctly, and the assertion below would pass on nothing.
        assert_ne!(
            value(&left, key),
            value(&right, key),
            "the two {slot} placements give one {key}, so this proves nothing about the digest"
        );
        assert_ne!(
            digest(&left, key),
            digest(&right, key),
            "two hands placed {slot} at two samples, {key} is {} and {}, and both fingerprint as \
             one result",
            value(&left, key),
            value(&right, key)
        );
    }
}

/// The other half, so the guard above cannot be satisfied by a build where every digest differs
/// from every other. Two runs that placed one landmark at one sample computed the number the
/// same way and say so.
#[test]
fn one_hand_placing_one_sample_twice_is_one_result() {
    for (slot, key, placed, repeated) in [
        (
            "onset",
            INTERVAL,
            request(Some(1180), None),
            request(Some(1180), None),
        ),
        (
            "takeoff",
            FLIGHT,
            request(None, Some(2360)),
            request(None, Some(2360)),
        ),
    ] {
        let (left, right) = (analysed(placed), analysed(repeated));
        assert_eq!(
            value(&left, key),
            value(&right, key),
            "the same {slot} placement gave two numbers"
        );
        assert_eq!(
            digest(&left, key),
            digest(&right, key),
            "one {slot} placement run twice fingerprints as two results"
        );
    }
}

/// The sample a hand placed reaches the record of the rule whose landmark it placed, and no
/// other rule's.
///
/// The hand places the sample the rule itself found, so both runs report the same number off the
/// same values, and the row is the only thing left saying which of the two a reader is holding.
///
/// Nothing here compares the two digests. They differ, and they differ for a reason that is not
/// the placement: a dragged marker rests on nothing, so the chain behind flight time loses the
/// weighing rule and runs 6 steps against the detection's 7. Asserting on that inequality would
/// read the thinning and report it as the placement, which is the state this whole entry exists
/// to correct. The placement's own effect on a digest is held where the chain can be pinned, in
/// `plateforce_core::reporting`'s own tests.
#[test]
fn the_sample_a_hand_placed_reaches_the_row_whose_landmark_it_placed() {
    let detected = analysed(request(None, None));
    let found_at = detected
        .takeoff_index
        .expect("the rule placed takeoff on this trace");
    let by_hand = analysed(request(None, Some(found_at)));

    assert_eq!(
        value(&detected, FLIGHT),
        value(&by_hand, FLIGHT),
        "the hand placed a different sample from the one the rule found, so the two runs differ \
         for a reason other than the one under test"
    );
    assert_eq!(
        rows_a_hand_placed(&by_hand),
        vec![(TAKEOFF_RULE.to_string(), found_at)],
        "the row a hand placed is not the only row carrying a sample, or is not the takeoff rule"
    );
    assert!(
        rows_a_hand_placed(&detected).is_empty(),
        "a run nobody touched claims a hand placed something"
    );
}
