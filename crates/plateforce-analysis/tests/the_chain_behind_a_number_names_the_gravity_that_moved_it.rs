//! A number the analysis gravity moves carries that gravity in its chain, and a number it does
//! not move carries none.
//!
//! The gravity belongs to the analysis and to no registry entry, so no rule may record it on its
//! own row and `chain_of` puts it on the root of the chain behind each number that rests on it.
//! Which numbers those are is a list in `chain.rs`, and a list is a claim: this measures the set
//! by moving the gravity and reading which numbers followed, and requires the two sets to be the
//! same set. A list that goes stale the day a twelfth rule starts reading gravity fails here
//! rather than passing while wrong.
//!
//! Both directions, because a guard reaching only for presence passes on a build that records a
//! gravity beside every number, including the ones integrated over an interval and divided by
//! nothing.
//!
//! The Python package held this alone, and held it for one surface. Every consumer reads the
//! tree this guards, so it is guarded where the tree is derived.

use std::collections::{BTreeMap, BTreeSet};

use plateforce_analysis::{
    chains_of, run, AnalysisRequest, AnalysisResponse, MethodChoice, WeighingChoice, GRAVITY_GLOBAL,
};
use plateforce_core::provenance::{ParameterSource, RegistryStamp};
use plateforce_core::reporting::fingerprint;
use plateforce_core::{Acquisition, Trial};

const SAMPLE_RATE_HZ: f64 = 1200.0;
const FLIGHT_SAMPLES: usize = 811;
const FLIGHT_TIME: &str = "flight_time_seconds";
const FLIGHT_TIME_HEIGHT: &str = "jump_height_from_flight_time_meters";

/// The two constants the tools argue over. Gravity varies by half a percent across the Earth's
/// surface, fifteen times this gap, so a guard that holds here holds on anything a plate owner
/// would state.
const STANDARD: f64 = 9.80665;
const PUBLISHED: f64 = 9.81;

/// A countermovement jump that leaves the plate and lands back on it, so every landmark is
/// placed, the flight time exists, and the rule that publishes its own gravity answers rather
/// than declining.
fn a_jump_that_lands() -> Trial {
    let mut force = vec![600.0; 1200];
    for (index, sample) in force.iter_mut().enumerate() {
        *sample += ((index % 17) as f64 - 8.0) * 0.4;
    }
    force.extend((0..240).map(|index| 600.0 - 220.0 * (index as f64 / 240.0)));
    force.extend((0..240).map(|index| 380.0 + 220.0 * (index as f64 / 240.0)));
    force.extend((0..660).map(|index| 600.0 + 900.0 * (index as f64 / 660.0)));
    force.extend(std::iter::repeat_n(0.0, FLIGHT_SAMPLES));
    force.extend(std::iter::repeat_n(2400.0, 240));
    force.extend(std::iter::repeat_n(600.0, 600));
    Trial::new(force, SAMPLE_RATE_HZ).unwrap()
}

fn base() -> AnalysisRequest {
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

/// The claim is written beside the value rather than through `state_gravity`, because these
/// guards need a value nobody chose and a value somebody chose at the same number.
fn at(gravity: f64, source: ParameterSource) -> AnalysisResponse {
    let mut request = base();
    request.gravity_meters_per_second_squared = gravity;
    request.gravity_source = source;
    run(&a_jump_that_lands(), &request).expect("the trace carries every landmark")
}

fn stamp() -> RegistryStamp {
    RegistryStamp {
        version: Some("fixture-pin".to_string()),
        declared_version: Some("fixture-declares".to_string()),
        digest: Some("content-fixture".to_string()),
    }
}

fn values(response: &AnalysisResponse) -> BTreeMap<String, Option<f64>> {
    response
        .metrics
        .iter()
        .map(|metric| (metric.key.clone(), metric.value))
        .collect()
}

/// The keys whose number is not the same at the two gravities. Computed, never listed.
fn moved_between(one: &AnalysisResponse, other: &AnalysisResponse) -> BTreeSet<String> {
    let (before, after) = (values(one), values(other));
    assert_eq!(
        before.keys().collect::<Vec<_>>(),
        after.keys().collect::<Vec<_>>(),
        "the two analyses reported different quantities, so nothing below compares numbers"
    );
    before
        .iter()
        .filter(|(key, value)| after[*key] != **value)
        .map(|(key, _)| key.clone())
        .collect()
}

/// Every step of one number's chain that names the analysis gravity, as (depth, value).
///
/// The whole chain rather than the root, so a record that reached a step nobody meant it to
/// reach is visible here rather than passing as the one at the top.
fn gravity_in_the_chain(response: &AnalysisResponse, key: &str) -> Vec<(usize, f64)> {
    let chains = chains_of(response, &stamp(), true);
    let derived = chains
        .iter()
        .find(|held| held.quantity == key)
        .unwrap_or_else(|| panic!("{key} is not among the quantities this analysis reported"));
    let mut found = Vec::new();
    let mut pending = vec![(0usize, &derived.chain)];
    while let Some((depth, step)) = pending.pop() {
        for parameter in &step.provenance.parameters {
            if parameter.name == GRAVITY_GLOBAL {
                found.push((depth, parameter.value));
            }
        }
        pending.extend(step.depends_on.iter().map(|below| (depth + 1, below)));
    }
    found
}

/// The keys whose chain names the analysis gravity anywhere.
fn naming_the_gravity(response: &AnalysisResponse) -> BTreeSet<String> {
    response
        .metrics
        .iter()
        .filter(|metric| metric.value.is_some())
        .filter(|metric| !gravity_in_the_chain(response, &metric.key).is_empty())
        .map(|metric| metric.key.clone())
        .collect()
}

/// The set that moves and the set that says so are one set.
///
/// Both halves are controlled: a build where gravity moved nothing would satisfy the equality
/// while proving none of it, and so would one where every number moved.
#[test]
fn every_number_the_analysis_gravity_moves_carries_it_in_its_chain() {
    let standard = at(STANDARD, ParameterSource::Stated);
    let published = at(PUBLISHED, ParameterSource::Stated);

    let moved = moved_between(&standard, &published);
    let reported: BTreeSet<String> = standard
        .metrics
        .iter()
        .filter(|metric| metric.value.is_some())
        .map(|metric| metric.key.clone())
        .collect();
    println!(
        "{} of {} numbers moved: {moved:?}",
        moved.len(),
        reported.len()
    );
    assert!(
        !moved.is_empty(),
        "no number moved between {STANDARD} and {PUBLISHED}, so nothing here is being tested"
    );
    assert!(
        moved.len() < reported.len(),
        "every number moved, so the half of this that reads absence is not being tested"
    );

    for (response, requested) in [(&standard, STANDARD), (&published, PUBLISHED)] {
        assert_eq!(
            naming_the_gravity(response),
            moved,
            "the numbers a moving gravity moves and the numbers whose chain names one are two \
             different sets at {requested}"
        );
    }
}

/// Once, at the root, and carrying the value the analysis ran at.
///
/// The depth matters: the record belongs to the number rather than to any rule, so it sits on
/// the step a reader meets first. A second copy further up would put one fact on two steps of
/// one tree, which is the shape this whole derivation exists to stop.
#[test]
fn the_gravity_a_number_names_is_recorded_once_at_the_root_and_is_the_one_it_ran_at() {
    for requested in [STANDARD, PUBLISHED] {
        let response = at(requested, ParameterSource::Stated);
        let named = naming_the_gravity(&response);
        assert!(
            !named.is_empty(),
            "no number named a gravity at {requested}"
        );
        for key in &named {
            let found = gravity_in_the_chain(&response, key);
            assert_eq!(
                found.len(),
                1,
                "{key} names the gravity {} times: {found:?}",
                found.len()
            );
            assert_eq!(
                found[0].0, 0,
                "{key} names the gravity at depth {}",
                found[0].0
            );
            assert_eq!(
                found[0].1, requested,
                "{key} ran at {requested} and its chain names {}",
                found[0].1
            );
        }
    }
}

/// A rule whose entry publishes its own gravity ran at that one, and the chain says so.
///
/// Nobody states a gravity, the request carries 9.80665, and `jumpheight.takeoff.flight_time`
/// runs at the 9.81 its entry declares. The record naming the request's value would name a
/// number that did not produce the height. Checked as the closed form rather than against a
/// second value read from the same place.
#[test]
fn a_rule_publishing_its_own_gravity_puts_the_one_it_ran_at_into_the_chain() {
    let quiet = at(STANDARD, ParameterSource::Assumed);
    let found = gravity_in_the_chain(&quiet, FLIGHT_TIME_HEIGHT);
    assert_eq!(found.len(), 1, "{FLIGHT_TIME_HEIGHT} names {found:?}");
    let recorded = found[0].1;
    assert_eq!(recorded, PUBLISHED);

    let read = |key: &str| {
        values(&quiet)
            .get(key)
            .copied()
            .flatten()
            .unwrap_or_else(|| panic!("{key} produced no number on a trace that lands"))
    };
    let flight = read(FLIGHT_TIME);
    let height = read(FLIGHT_TIME_HEIGHT);
    assert!(
        (height - recorded * flight * flight / 8.0).abs() < 1e-12,
        "the height is {height} and the recorded {recorded} gives {}",
        recorded * flight * flight / 8.0
    );
    assert!(
        (height - STANDARD * flight * flight / 8.0).abs() > 1e-9,
        "the two gravities give the same height, so this proves nothing about which was used"
    );
}

/// Two numbers that are not the same number do not fingerprint as one result.
///
/// The fingerprint is taken over the chain, so a value the chain does not carry cannot reach
/// it. Four of the five quantities here moved between two gravities and fingerprinted alike,
/// which is the strongest form the defect took: a reader comparing two results against each
/// other was told they matched.
///
/// The plate is fully recorded, because an incomplete acquisition block matches nothing,
/// itself included, and two incomplete fingerprints compare equal whatever the digests are.
#[test]
fn two_gravities_that_move_a_number_give_it_two_fingerprints() {
    let standard = at(STANDARD, ParameterSource::Stated);
    let published = at(PUBLISHED, ParameterSource::Stated);
    let moved = moved_between(&standard, &published);
    assert!(
        !moved.is_empty(),
        "no number moved, so nothing here is being fingerprinted twice"
    );

    let taken = |response: &AnalysisResponse, key: &str| {
        let chains = chains_of(response, &stamp(), true);
        let derived = chains
            .iter()
            .find(|held| held.quantity == key)
            .unwrap_or_else(|| panic!("{key} has no chain"));
        fingerprint(
            &derived.chain.provenance,
            &a_recorded_plate(),
            SAMPLE_RATE_HZ,
        )
    };

    for key in &moved {
        let one = taken(&standard, key);
        assert!(one.complete, "a recorded plate publishes a digest");
        assert_ne!(
            one,
            taken(&published, key),
            "{key} is a different number at {STANDARD} and at {PUBLISHED} and fingerprints the \
             same, so two results that do not match would be declared to"
        );
    }

    // The control on the loop above. A fingerprint that moved for every quantity would satisfy
    // it while telling a reader that two identical numbers are two results.
    let still: Vec<&String> = standard
        .metrics
        .iter()
        .filter(|metric| metric.value.is_some())
        .map(|metric| &metric.key)
        .filter(|key| !moved.contains(*key))
        .collect();
    assert!(
        !still.is_empty(),
        "every number moved, so nothing held still"
    );
    for key in still {
        assert_eq!(
            taken(&standard, key),
            taken(&published, key),
            "{key} is the same number at both gravities and fingerprints as two results"
        );
    }
}

/// A plate whose settings were all recorded, so a fingerprint taken over it is one that
/// publishes. An unfilled block matches nothing, itself included, so a comparison over two
/// incomplete fingerprints passes whatever the digests are.
fn a_recorded_plate() -> Acquisition {
    Acquisition {
        filter_at_capture: Some("none".to_string()),
        tare_state: Some("tared_before_trial".to_string()),
        plate_natural_frequency_hz: Some(400.0),
        floor_surface: Some("concrete".to_string()),
        firmware_version: Some("2.4.1".to_string()),
    }
}
