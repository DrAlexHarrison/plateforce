//! The same question `the_chain_behind_a_number_names_the_gravity_that_moved_it` asks, asked of
//! every rule this build runs rather than of the first rule filed under each construct.
//!
//! That guard reads its population off `derived_bindings()` and keeps the first entry per
//! construct, which is the right population for the question it asks and is not the whole build.
//! Measured on subject 01's first trial, the two rules filed under `propulsion_phase_start`
//! answer differently:
//!
//! ```text
//! phase.propulsion_start.zero_velocity        3.9333333333333336 s at 9.0 and at 11.0
//! phase.propulsion_start.velocity_threshold   3.935 s at 9.0, 3.934166666666667 s at 9.80665
//! ```
//!
//! Both read the analysis gravity. The first takes the zero crossing of a velocity series
//! scaled by `1/g`, and scaling a series moves neither its zeros nor its extrema, so its
//! boundary is the same sample at any gravity. The second compares that series against a
//! threshold in metres per second, which is not scale invariant, so its boundary moves.
//! `zero_velocity` is declared first, so the narrower guard binds it and passes for a reason
//! other than the one it states: it is green because the rule it reached cannot move.
//!
//! One rule at a time rather than all of them at once, because a request binding every rule
//! reports one number per construct and the rules that lost the slot are not measured at all.

use std::collections::{BTreeMap, BTreeSet};

use plateforce_analysis::binding::derived_bindings;
use plateforce_analysis::{
    chains_of, run, AnalysisRequest, AnalysisResponse, MethodChoice, WeighingChoice, GRAVITY_GLOBAL,
};
use plateforce_core::provenance::{ParameterSource, RegistryStamp};
use plateforce_core::Trial;

mod common;

const SAMPLE_RATE_HZ: f64 = 1200.0;

/// Wider than the two constants the tools argue over, because this is asking whether a rule's
/// number rests on the value at all rather than how far apart two published constants are. A
/// boundary that sits on the same sample at 9.0 and at 11.0 rests on nothing a plate owner
/// could state.
const LOW: f64 = 9.0;
const HIGH: f64 = 11.0;

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

/// The spine, the first rule under every construct so the prerequisites are placed, and then
/// `under_test` swapped into its own construct.
///
/// The prerequisite set is the narrower guard's population, which is what makes this a
/// widening of it rather than a second question.
fn naming(under_test: &plateforce_analysis::Binding, gravity: f64) -> AnalysisRequest {
    let mut derived: BTreeMap<String, MethodChoice> = BTreeMap::new();
    for binding in derived_bindings() {
        derived
            .entry(binding.construct.to_string())
            .or_insert_with(|| MethodChoice {
                method_id: binding.id.to_string(),
                ..Default::default()
            });
    }
    derived.insert(
        under_test.construct.to_string(),
        MethodChoice {
            method_id: under_test.id.to_string(),
            ..Default::default()
        },
    );
    common::prepared(AnalysisRequest {
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
        derived,
        gravity_meters_per_second_squared: gravity,
        gravity_source: ParameterSource::Stated,
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

/// The keys whose chain names the analysis gravity anywhere, over the numbers this response
/// produced. A rule that declined reports no value and leaves nothing to compare.
fn naming_the_gravity(response: &AnalysisResponse) -> BTreeSet<String> {
    let chains = chains_of(response, &stamp(), true);
    response
        .metrics
        .iter()
        .filter(|metric| metric.value.is_some())
        .filter(|metric| {
            let Some(held) = chains.iter().find(|held| held.quantity == metric.key) else {
                return false;
            };
            let mut pending = vec![&held.chain];
            while let Some(step) = pending.pop() {
                if step
                    .provenance
                    .parameters
                    .iter()
                    .any(|parameter| parameter.name == GRAVITY_GLOBAL)
                {
                    return true;
                }
                pending.extend(step.depends_on.iter());
            }
            false
        })
        .map(|metric| metric.key.clone())
        .collect()
}

fn values(response: &AnalysisResponse) -> BTreeMap<String, Option<f64>> {
    response
        .metrics
        .iter()
        .map(|metric| (metric.key.clone(), metric.value))
        .collect()
}

/// For every rule this build runs, the numbers a moving gravity moves and the numbers whose
/// chain names one are the same set.
///
/// Controlled on both sides at the population level rather than per rule, because most rules
/// move nothing and a per-rule "something moved" assertion would fail on the honest ones. The
/// two controls are that at least one rule moved a number and at least one number held still,
/// so neither half of the equality is satisfied by a build where nothing happens.
#[test]
fn every_rule_that_gravity_moves_records_it_and_every_rule_it_does_not_records_nothing() {
    let trial = a_jump_that_lands();
    let mut rules_whose_numbers_moved = Vec::new();
    let mut rules_reached = 0usize;
    let mut out_of_reach: Vec<&str> = Vec::new();
    let mut numbers_that_held_still = 0usize;
    let mut disagreements = Vec::new();

    for binding in derived_bindings() {
        let low = run(&trial, &naming(binding, LOW)).expect("the trace carries every landmark");
        let high = run(&trial, &naming(binding, HIGH)).expect("the trace carries every landmark");
        let (before, after) = (values(&low), values(&high));
        // A rule reporting a different set of keys at the two gravities would put itself out
        // of reach of everything below, so it is counted rather than skipped in silence: a
        // guard whose population quietly shrinks is the shape this file exists to answer.
        if before.keys().ne(after.keys()) {
            out_of_reach.push(binding.id);
            continue;
        }
        rules_reached += 1;

        let moved: BTreeSet<String> = before
            .iter()
            .filter(|(key, value)| after[*key] != **value)
            .map(|(key, _)| key.clone())
            .collect();
        numbers_that_held_still += before.len() - moved.len();
        if !moved.is_empty() {
            rules_whose_numbers_moved.push(binding.id);
        }
        let named = naming_the_gravity(&low);
        if named != moved {
            disagreements.push(format!(
                "{}: moves {moved:?} and names {named:?}",
                binding.id
            ));
        }
    }

    let offered = derived_bindings().count();
    println!(
        "{rules_reached} of {offered} rules reached, {} of them move a number when the gravity \
         moves, {numbers_that_held_still} number-readings held still",
        rules_whose_numbers_moved.len()
    );
    assert!(
        out_of_reach.is_empty(),
        "{} of {offered} rules reported a different set of quantities at {LOW} and at {HIGH}, \
         so this measured nothing about them: {out_of_reach:?}",
        out_of_reach.len()
    );
    assert!(
        !rules_whose_numbers_moved.is_empty(),
        "no rule moved a number between {LOW} and {HIGH}, so nothing here is being tested"
    );
    assert!(
        numbers_that_held_still > 0,
        "every number moved, so the half of this that reads absence is not being tested"
    );
    assert!(
        disagreements.is_empty(),
        "a rule's number moves with the analysis gravity and its chain does not say so, or \
         says so about a number that held still:\n  {}",
        disagreements.join("\n  ")
    );
}

/// The two rules under one construct that answer differently, held directly.
///
/// The general assertion above would go green again if both rules were declared in the other
/// order, or if the first-per-construct population happened to reach the moving one. This
/// names the pair, so the case that was missed stays reachable by name.
#[test]
fn two_rules_for_one_boundary_disagree_about_whether_gravity_moves_it() {
    let trial = a_jump_that_lands();
    let boundary_at = |method_id: &str, gravity: f64| {
        let binding = derived_bindings()
            .find(|binding| binding.id == method_id)
            .unwrap_or_else(|| panic!("{method_id} is not a rule this build runs"));
        run(&trial, &naming(binding, gravity))
            .expect("the trace carries every landmark")
            .metric("propulsion_phase_start_seconds")
            .and_then(|metric| metric.value)
            .unwrap_or_else(|| panic!("{method_id} placed no propulsion phase start"))
    };

    let scaled = "phase.propulsion_start.zero_velocity";
    let thresholded = "phase.propulsion_start.velocity_threshold";
    assert_eq!(
        boundary_at(scaled, LOW),
        boundary_at(scaled, HIGH),
        "{scaled} takes the zero crossing of a series scaled by 1/g, and scaling a series does \
         not move its zeros, so its boundary cannot rest on the gravity"
    );
    assert_ne!(
        boundary_at(thresholded, LOW),
        boundary_at(thresholded, HIGH),
        "{thresholded} compares that series against a threshold in meters per second, which is \
         not scale invariant, so its boundary rests on the gravity"
    );
}
