//! A quantity the landmarks produce directly roots its chain at the rule filling its own
//! construct, whatever else fed the number and whatever order the record lists the rules in.
//!
//! An onset search bounded by takeoff carries the takeoff rule among the onset time's
//! contributors. A root chosen by where a rule sits in that list follows the takeoff rule,
//! because the onset and the takeoff rules run in the same phase and the later one wins. The
//! record then reads as though takeoff produced the onset, with the onset rule demoted to one
//! of its inputs.
//!
//! The rooting cases reverse the contributor order and assert the same root, so a build that
//! reintroduced an ordering fails one direction rather than passing whichever way the response
//! happens to list ids today. Taking the tie from the earliest end instead of the latest is a
//! build the forward direction alone reports as correct.

use std::collections::BTreeMap;

use plateforce_analysis::chain::chain_of;
use plateforce_analysis::response::Metric;
use plateforce_analysis::{
    run, AnalysisRequest, AnalysisResponse, MethodChoice, WeighingChoice, ONSET_CONSTRUCT,
};
use plateforce_core::provenance::RegistryStamp;
use plateforce_core::Trial;

mod common;

const SAMPLE_RATE_HZ: f64 = 1200.0;

/// A countermovement jump that leaves the plate and lands back on it, so every landmark is
/// placed and the onset search has a takeoff to bound itself against.
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

fn stamp() -> RegistryStamp {
    RegistryStamp {
        version: Some("fixture-pin".to_string()),
        declared_version: Some("fixture-declares".to_string()),
        digest: Some("content-fixture".to_string()),
    }
}

/// A run under a stated onset rule and a stated takeoff rule.
fn analysed(onset: &str, takeoff: &str) -> AnalysisResponse {
    let request = common::prepared(AnalysisRequest {
        weighing: WeighingChoice {
            method_id: "bwepoch.fixed_window".into(),
            parameters: BTreeMap::from([("duration".to_string(), 0.8)]),
            ..Default::default()
        },
        onset: MethodChoice {
            method_id: onset.into(),
            ..Default::default()
        },
        takeoff: MethodChoice {
            method_id: takeoff.into(),
            ..Default::default()
        },
        ..Default::default()
    });
    run(&a_jump_that_lands(), &request).expect("the request is well formed")
}

fn metric_for<'a>(response: &'a AnalysisResponse, key: &str) -> &'a Metric {
    response
        .metric(key)
        .unwrap_or_else(|| panic!("{key} is not reported, so there is no chain to read"))
}

fn root_of(response: &AnalysisResponse, metric: &Metric) -> String {
    chain_of(response, metric, &stamp(), true)
        .provenance
        .method_id
}

/// The same metric with the rules its answer rests on listed backwards.
///
/// The record's order is what the old root read, so a build that still reads it answers these
/// two differently. Nothing else about the number changes.
fn listed_backwards(metric: &Metric) -> Metric {
    let mut reversed = metric.clone();
    reversed.contributing_method_ids.reverse();
    reversed
}

/// The onset time roots at the onset rule on every pairing of onset and takeoff rules, and it
/// roots there in both list directions.
#[test]
fn the_onset_time_roots_at_the_onset_rule_whichever_way_the_rules_are_listed() {
    let onsets = [
        "onset.threshold.noise_relative",
        "onset.threshold.last_within_band",
        "onset.threshold.adaptive_trailing_window",
    ];
    let takeoffs = [
        "takeoff.threshold.absolute_force",
        "takeoff.threshold.flight_noise_k_sd",
        "takeoff.threshold.landing_shape",
    ];

    let mut checked = 0usize;
    let mut carried_the_takeoff_rule = 0usize;
    for onset in onsets {
        for takeoff in takeoffs {
            let response = analysed(onset, takeoff);
            let metric = metric_for(&response, "onset_time_seconds");
            // The case this file exists for. A pairing whose onset never reads takeoff cannot
            // tell a root by construct from a root by position, and the count below is how
            // many pairings could.
            if metric
                .contributing_method_ids
                .iter()
                .any(|id| id.starts_with("takeoff."))
            {
                carried_the_takeoff_rule += 1;
            }
            checked += 1;

            let forwards = root_of(&response, metric);
            let backwards = root_of(&response, &listed_backwards(metric));
            for (direction, root) in [("forwards", &forwards), ("backwards", &backwards)] {
                assert!(
                    root.starts_with("onset."),
                    "{onset} with {takeoff} listed {direction} roots the onset time at {root}"
                );
            }
            assert_eq!(
                forwards, backwards,
                "{onset} with {takeoff} roots the onset time at two rules depending on the order \
                 the record lists them in"
            );
        }
    }

    assert_eq!(checked, 9, "the sweep read {checked} pairings");
    assert!(
        carried_the_takeoff_rule > 0,
        "no pairing put the takeoff rule among the onset time's contributors, so this case \
         could not tell a root by construct from a root by position"
    );
    println!(
        "{carried_the_takeoff_rule} of {checked} pairings carry the takeoff rule among the onset \
         time's contributors, and all {checked} root at the onset rule in both directions"
    );
}

/// The takeoff time and the weighing quantities root at their own rules too, in both
/// directions.
///
/// The takeoff was right before this fix only because the onset rule is absent from its
/// contributors, which is a fact about that chain rather than the root working.
#[test]
fn every_landmark_quantity_roots_at_its_own_rule() {
    let response = analysed(
        "onset.threshold.last_within_band",
        "takeoff.threshold.flight_noise_k_sd",
    );
    let owed = [
        ("system_weight_newtons", "bwepoch."),
        ("system_mass_kilograms", "bwepoch."),
        ("onset_time_seconds", "onset."),
        ("takeoff_time_seconds", "takeoff."),
    ];
    let mut rooted: Vec<String> = Vec::new();
    for (key, slot) in owed {
        let metric = metric_for(&response, key);
        for (direction, held) in [
            ("forwards", metric.clone()),
            ("backwards", listed_backwards(metric)),
        ] {
            let root = root_of(&response, &held);
            assert!(
                root.starts_with(slot),
                "{key} listed {direction} roots at {root}, which does not fill {slot}"
            );
            if direction == "forwards" {
                rooted.push(root);
            }
        }
    }

    // Three constructs ran under three rules, so the four quantities name three rules. The two
    // weighing quantities are one rule twice, and a build that collapsed the onset onto the
    // takeoff would name two.
    let mut distinct = rooted.clone();
    distinct.sort();
    distinct.dedup();
    assert_eq!(
        distinct.len(),
        3,
        "four landmark quantities ran under three rules and rooted at {}: {distinct:?}",
        distinct.len()
    );
}

/// Every quantity naming no arithmetic entry declares the construct that produces it, and
/// every quantity naming one declares no construct.
///
/// The declaration is what roots a landmark chain, and a row added without it compiles: the
/// field has to be written, and `None` is a value. A quantity that named neither would fall to
/// the ranking, which is what this fix removed, and the fall would be silent.
#[test]
fn a_quantity_names_an_arithmetic_entry_or_the_construct_that_produces_it() {
    let mut landmarks = 0usize;
    let mut silent: Vec<&str> = Vec::new();
    let mut both: Vec<&str> = Vec::new();
    for quantity in plateforce_analysis::response::QUANTITIES.iter() {
        match (quantity.computed_by, quantity.produced_by_construct) {
            (None, None) => silent.push(quantity.key),
            (Some(_), Some(_)) => both.push(quantity.key),
            (None, Some(_)) => landmarks += 1,
            (Some(_), None) => {}
        }
    }
    let declared = plateforce_analysis::response::QUANTITIES.len();
    assert!(
        silent.is_empty(),
        "{} of {declared} quantities name neither an arithmetic entry nor a construct: {silent:?}",
        silent.len()
    );
    assert!(
        both.is_empty(),
        "{} of {declared} quantities name an arithmetic entry and a construct: {both:?}",
        both.len()
    );
    assert_eq!(
        landmarks, 4,
        "{landmarks} of {declared} quantities are produced by a landmark rule directly"
    );
}

/// A quantity that declares a construct and finds no rule filling it roots at nothing.
///
/// The alternative is a fallback to the ranking, which cannot tell an onset rule from a
/// takeoff rule and is the ordering this fix removed. A chain that quietly acquired a root
/// from it would report a rule that did not produce the number, which is the defect again with
/// a longer fuse.
#[test]
fn a_declared_construct_that_no_rule_filled_roots_at_no_rule() {
    let response = analysed(
        "onset.threshold.last_within_band",
        "takeoff.threshold.flight_noise_k_sd",
    );
    let metric = metric_for(&response, "onset_time_seconds");

    // The same number resting on everything except a rule filling its own construct. The
    // takeoff rule and the weighing rule are still there, and both outrank nothing.
    let mut without_the_onset_rule = metric.clone();
    without_the_onset_rule
        .contributing_method_ids
        .retain(|id| !id.starts_with("onset."));
    assert!(
        without_the_onset_rule
            .contributing_method_ids
            .iter()
            .any(|id| id.starts_with("takeoff.")),
        "the takeoff rule is what a ranking would reach for, so this case needs it present"
    );

    let root = root_of(&response, &without_the_onset_rule);
    assert!(
        root.is_empty(),
        "the onset time rooted at {root} with no {ONSET_CONSTRUCT} rule among its contributors"
    );
}
