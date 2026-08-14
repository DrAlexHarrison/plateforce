//! A sweep that reports no number for a combination says which rule that number is missing on
//! account of.
//!
//! The panel's whole claim is how far a quantity moves across the alternatives the literature
//! contains. A variant with no number and no reason gives a reader nothing to judge: they
//! cannot tell a combination this recording cannot answer from one the software declined to
//! run, and the count in the denominator is the same either way.
//!
//! Why it read empty. Which rules a number rests on is one question with one answer, and the
//! sweep had a second copy of it that read the landmark chain alone. A quantity's own
//! arithmetic is not on that list: it is what `computed_by` names. So a decline by the rule
//! that computes the number was invisible to the panel, which is every decline that matters
//! for a quantity the landmarks reach.
//!
//! Measured on this tree before the change, sweeping `jump_height_from_flight_time_meters` over
//! the three weighing rules by the five onset rules by the five takeoff rules on
//! `subject01_trial2`: 75 of 75 came back with no number and **15** of them carried a reason.
//! Reading the whole chain reaches 75.
//!
//! The denominators here are read off the binding table rather than written down, so a rule
//! added to the build widens this sweep on the day it is added.

use std::collections::BTreeMap;

use plateforce_analysis::spread::{Axis, SpreadRequest, SpreadResponse};
use plateforce_analysis::{chain_names, run, AnalysisRequest, MethodChoice, WeighingChoice};
use plateforce_core::{read_trial_from_path, Trial};

mod common;

const FIXTURES: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../plateforce-conformance/fixtures/"
);
const CORPUS_SAMPLE_RATE_HZ: f64 = 1200.0;

/// The height taken from the flight time, which is the quantity whose arithmetic is a rule of
/// its own rather than a step on the landmark chain.
const QUANTITY: &str = "jump_height_from_flight_time_meters";

/// The recording that stops while the athlete is still in the air, so the rule measuring to the
/// return to the plate declines whatever landmarks are placed. Five of the six subject
/// recordings do this, which is a property of the corpus rather than of the software.
const STOPS_IN_FLIGHT: &str = "subject01_trial2";

/// The one recording in the corpus that comes back down, so every combination answers. The
/// control: a build writing a reason on every variant passes the first guard and fails here.
const LANDS: &str = "subject01_trial1";

fn trial(name: &str) -> Trial {
    let path = format!("{FIXTURES}{name}.force.txt");
    let (trial, _) = read_trial_from_path(&path, '\t', 0, CORPUS_SAMPLE_RATE_HZ)
        .unwrap_or_else(|error| panic!("{path} did not read: {error}"));
    trial
}

/// Every rule the build files under a construct, which is what the axis compares.
fn rules_for(construct: &str) -> Vec<String> {
    plateforce_analysis::BINDINGS
        .iter()
        .filter(|binding| binding.construct == construct)
        .map(|binding| binding.id.to_string())
        .collect()
}

fn base() -> AnalysisRequest {
    common::prepared(AnalysisRequest {
        weighing: WeighingChoice {
            method_id: "bwepoch.fixed_window".into(),
            parameters: BTreeMap::from([
                ("duration".to_string(), 1.0),
                ("window_seconds".to_string(), 1.0),
            ]),
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

/// The three landmark slots swept across every rule the build files under each.
fn every_landmark_rule() -> SpreadRequest {
    SpreadRequest {
        base: base(),
        axes: vec![
            Axis {
                slot: "weighing".into(),
                method_ids: rules_for(plateforce_analysis::WEIGHING_CONSTRUCT),
                ..Default::default()
            },
            Axis {
                slot: "onset".into(),
                method_ids: rules_for(plateforce_analysis::ONSET_CONSTRUCT),
                ..Default::default()
            },
            Axis {
                slot: "takeoff".into(),
                method_ids: rules_for(plateforce_analysis::TAKEOFF_CONSTRUCT),
                ..Default::default()
            },
        ],
        quantity_key: QUANTITY.to_string(),
        maximum_combinations: 512,
    }
}

fn swept(name: &str) -> SpreadResponse {
    plateforce_analysis::spread::run(&trial(name), &every_landmark_rule())
        .unwrap_or_else(|refusal| panic!("the sweep ran: {refusal}"))
}

/// The product of the three axes, read off the binding table so the denominator moves with the
/// build rather than with this file.
fn combinations() -> usize {
    rules_for(plateforce_analysis::WEIGHING_CONSTRUCT).len()
        * rules_for(plateforce_analysis::ONSET_CONSTRUCT).len()
        * rules_for(plateforce_analysis::TAKEOFF_CONSTRUCT).len()
}

/// Every variant that came back without a number says why.
///
/// Both counts are stated. The empty count is what makes the reasoned count readable: 75 of 75
/// carrying a reason means nothing without knowing that 75 of 75 came back empty, and a build
/// that answered every variant would report 0 of 0 and satisfy any assertion phrased as an
/// absence of unexplained variants.
#[test]
fn every_swept_variant_with_no_number_carries_the_reason_that_number_is_absent() {
    let response = swept(STOPS_IN_FLIGHT);
    let denominator = combinations();
    assert_eq!(response.variants.len(), denominator);
    assert_eq!(response.combinations_run, denominator);

    let empty: Vec<&plateforce_analysis::spread::Variant> = response
        .variants
        .iter()
        .filter(|variant| variant.value.is_none())
        .collect();
    let unexplained: Vec<&str> = empty
        .iter()
        .filter(|variant| variant.failure_reason.is_none())
        .map(|variant| variant.label.as_str())
        .collect();

    println!(
        "{} of {denominator} variants came back with no number, {} of them unexplained",
        empty.len(),
        unexplained.len()
    );
    assert_eq!(
        empty.len(),
        denominator,
        "the recording no longer leaves every combination without a number, so this is not the \
         population the reason is measured over"
    );
    assert!(
        unexplained.is_empty(),
        "{} of {} variants report no value and no reason: {unexplained:?}",
        unexplained.len(),
        empty.len()
    );
    // The whole-sweep figures agree with the variants, so a reader is not told a spread was
    // taken over numbers the list does not carry.
    assert_eq!(response.succeeded, 0);
    assert_eq!(response.failed, denominator);
}

/// A variant that produced a number carries no reason.
///
/// The control, and it is the one that has to discriminate: a build attaching the analysis's
/// first refusal to every variant satisfies the guard above completely, and a reader would meet
/// a reason under a number that is sitting in front of them.
#[test]
fn a_swept_variant_that_produced_a_number_carries_no_reason() {
    let response = swept(LANDS);
    let denominator = combinations();
    let answered = response
        .variants
        .iter()
        .filter(|variant| variant.value.is_some())
        .count();
    let claimed: Vec<&str> = response
        .variants
        .iter()
        .filter(|variant| variant.value.is_some() && variant.failure_reason.is_some())
        .map(|variant| variant.label.as_str())
        .collect();

    println!(
        "{answered} of {denominator} variants answered, {} of them carry a reason anyway",
        claimed.len()
    );
    assert_eq!(
        answered, denominator,
        "the control recording no longer answers every combination, so it can no longer tell a \
         reason written on every variant from one written on the empty ones"
    );
    assert!(
        claimed.is_empty(),
        "a reason is written against a variant that produced a number: {claimed:?}"
    );
}

/// The reason a variant gives is a rule that declined on that variant's own chain.
///
/// Each empty variant is run again by itself and the refusal it named is held against the
/// quantity's own chain in that run. A reason lifted from a rule the number had no part in
/// would tell a reader to change a rule that was working, which is the failure the quality
/// signal on this project was rebuilt to stop.
#[test]
fn the_reason_a_variant_gives_names_a_rule_on_that_variants_own_chain() {
    let response = swept(STOPS_IN_FLIGHT);
    let recording = trial(STOPS_IN_FLIGHT);
    let mut checked = 0usize;
    let mut foreign: Vec<String> = Vec::new();

    for variant in &response.variants {
        let Some(reason) = &variant.failure_reason else {
            continue;
        };
        // The variant's own three rules, in the order the sweep records them.
        let [weighing, onset, takeoff] = variant.method_ids.as_slice() else {
            panic!("a variant names {} rules", variant.method_ids.len());
        };
        let mut request = base();
        request.weighing.method_id = weighing.clone();
        request.onset.method_id = onset.clone();
        request.takeoff.method_id = takeoff.clone();
        let request = common::prepared(request);

        let Ok(rerun) = run(&recording, &request) else {
            continue;
        };
        let metric = rerun
            .metric(QUANTITY)
            .unwrap_or_else(|| panic!("{QUANTITY} is absent under {}", variant.label));
        checked += 1;
        assert!(
            metric.value.is_none(),
            "{} answers when it is run on its own and came back empty in the sweep",
            variant.label
        );
        if !chain_names(metric, &reason.method_id) {
            foreign.push(format!(
                "{} blames {} which is not on the chain {:?} nor its arithmetic {:?}",
                variant.label, reason.method_id, metric.contributing_method_ids, metric.computed_by
            ));
        }
    }

    println!(
        "{checked} of {} variants re-run and their reason held against their own chain",
        response.variants.len()
    );
    assert_eq!(
        checked,
        combinations(),
        "some variants could not be re-run, so the check covered fewer than the sweep reported"
    );
    assert!(
        foreign.is_empty(),
        "a reason is written from a rule the number does not rest on: {foreign:?}"
    );
}

/// The half a second copy of this question had dropped, named rather than left to the counts.
///
/// The rule that declines here is the quantity's own arithmetic, so it is what `computed_by`
/// names and it is absent from `contributing_method_ids`. An attribution reading the landmark
/// chain alone finds nothing to say on any of these variants.
#[test]
fn the_rule_a_variant_blames_is_reachable_only_through_the_arithmetic_that_computed_it() {
    let response = swept(STOPS_IN_FLIGHT);
    let recording = trial(STOPS_IN_FLIGHT);
    let mut through_the_arithmetic = 0usize;

    for variant in &response.variants {
        let Some(reason) = &variant.failure_reason else {
            continue;
        };
        let [weighing, onset, takeoff] = variant.method_ids.as_slice() else {
            panic!("a variant names {} rules", variant.method_ids.len());
        };
        let mut request = base();
        request.weighing.method_id = weighing.clone();
        request.onset.method_id = onset.clone();
        request.takeoff.method_id = takeoff.clone();
        let Ok(rerun) = run(&recording, &common::prepared(request)) else {
            continue;
        };
        let metric = rerun.metric(QUANTITY).expect("the quantity is reported");
        let on_the_landmark_chain = metric
            .contributing_method_ids
            .iter()
            .any(|id| *id == reason.method_id);
        if !on_the_landmark_chain {
            assert_eq!(
                metric.computed_by.as_deref(),
                Some(reason.method_id.as_str()),
                "{} blames a rule that is neither on the chain nor the arithmetic",
                variant.label
            );
            through_the_arithmetic += 1;
        }
    }

    println!(
        "{through_the_arithmetic} of {} reasons are reachable only through the arithmetic",
        response.variants.len()
    );
    assert!(
        through_the_arithmetic > 0,
        "every reason on this sweep is on the landmark chain, so the sweep can no longer tell an \
         attribution that reads the whole chain from one that reads half of it"
    );
}
