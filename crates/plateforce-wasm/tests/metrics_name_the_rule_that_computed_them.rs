//! A number's provenance has to include the arithmetic, not only the landmarks it rests on.
//!
//! Every metric already reports which landmark rules fed it. That is not the same as reporting
//! which rule turned those landmarks into the number, and the gap hid a real defect: modified
//! reactive strength has two registry entries differing in their numerator, the registry marks the
//! choice `force_a_decision`, and this build resolved it silently and emitted the result under an
//! id that resolved to nothing.
//!
//! So `computed_by` names a registry entry, and this file holds it to that. `None` is allowed,
//! because some quantities genuinely have no entry describing their arithmetic, but a name that
//! does not resolve is not.

use std::collections::BTreeMap;

use plateforce_analysis::{run, AnalysisRequest, MethodChoice, WeighingChoice};
use plateforce_core::STANDARD_GRAVITY_METERS_PER_SECOND_SQUARED;
use plateforce_wasm::demo::synthetic_countermovement_jump;
use plateforce_wasm::registry_embed;

mod common;

fn request() -> AnalysisRequest {
    common::prepared(AnalysisRequest {
        weighing: WeighingChoice {
            method_id: "bwepoch.fixed_window".into(),
            start_index: None,
            parameters: BTreeMap::from([("duration".to_string(), 1.0)]),
            options: BTreeMap::new(),
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
        touchdown_index: None,
        gravity_meters_per_second_squared: STANDARD_GRAVITY_METERS_PER_SECOND_SQUARED,
        registry_backed_ids: Vec::new(),
        ..Default::default()
    })
}

#[test]
fn every_named_computation_resolves_in_the_registry() {
    let loaded = registry_embed::load().expect("a registry file did not parse");
    let trial = synthetic_countermovement_jump();
    let response = run(&trial, &request()).expect("the rules ran");

    let mut named = 0usize;
    let mut unresolved = Vec::new();
    for metric in &response.metrics {
        let Some(id) = metric.computed_by.as_deref() else {
            continue;
        };
        named += 1;
        if !loaded.registry.methods.contains_key(id) {
            unresolved.push(format!("{} names {id}", metric.key));
        }
    }

    assert!(
        unresolved.is_empty(),
        "a metric names arithmetic the registry does not carry, so its provenance cannot be \
         looked up:\n  {}",
        unresolved.join("\n  ")
    );
    assert!(
        named >= 3,
        "only {named} metrics name the rule that computed them, so this guard has stopped \
         covering the result"
    );
}

/// The one that motivated the field. Two numerators, a genuine debate between them, and the
/// registry says the choice must be forced rather than defaulted.
#[test]
fn modified_reactive_strength_says_which_numerator_it_used() {
    let loaded = registry_embed::load().expect("a registry file did not parse");
    let trial = synthetic_countermovement_jump();
    let response = run(&trial, &request()).expect("the rules ran");

    let rsi = response
        .metrics
        .iter()
        .find(|metric| metric.key == "reactive_strength_index_modified")
        .expect("the build reports modified reactive strength");

    assert_eq!(rsi.computed_by.as_deref(), Some("rsimod.jh_tov_over_ttt"));

    let alternative = "rsimod.jh_ft_over_ttt";
    assert!(
        loaded.registry.methods.contains_key(alternative),
        "the second numerator is missing from the registry, so the choice this build makes is no \
         longer visible as a choice"
    );
    assert!(
        rsi.note
            .as_deref()
            .is_some_and(|note| note.contains(alternative)),
        "the reader is not told the other numerator exists, which is what makes a forced decision \
         look like an observation"
    );
}
