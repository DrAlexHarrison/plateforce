//! Which operators run when a caller states nothing, and what the registry says about each.
//!
//! `ONSET_OPERATOR_IDS` lists what this build can compose, not what it does compose, and the
//! two were read as one. Composing an operator is a choice made on the user's behalf, so the
//! set that actually runs on a bare request is pinned here rather than inferred from the list.

use std::collections::BTreeMap;

use plateforce_analysis::{run, AnalysisRequest, MethodChoice, WeighingChoice};
use plateforce_core::{Trial, STANDARD_GRAVITY_METERS_PER_SECOND_SQUARED};
use plateforce_registry::schema::Surfacing;
use plateforce_registry::Registry;

const SAMPLE_RATE_HZ: f64 = 1200.0;

fn registry() -> Registry {
    Registry::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../registry"))
        .expect("the shipped registry loads")
}

/// Quiet stance, an unweighting dip, a push, flight, then landing.
fn trial() -> Trial {
    let mut force = vec![600.0; 1200];
    for (index, value) in force.iter_mut().enumerate() {
        *value += ((index % 17) as f64 - 8.0) * 0.4;
    }
    force.extend((0..360).map(|index| 600.0 - 300.0 * (index as f64 / 360.0)));
    force.extend((0..360).map(|index| 300.0 + 1200.0 * (index as f64 / 360.0)));
    force.extend(std::iter::repeat_n(0.0, 600));
    force.extend(std::iter::repeat_n(1400.0, 240));
    Trial::new(force, SAMPLE_RATE_HZ).unwrap()
}

/// Nothing stated anywhere: the request a surface sends before a user has touched anything.
fn bare_request() -> AnalysisRequest {
    AnalysisRequest {
        weighing: WeighingChoice {
            method_id: "bwepoch.fixed_window".into(),
            start_index: None,
            parameters: BTreeMap::from([("duration".to_string(), 1.0)]),
            options: BTreeMap::new(),
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
    }
}

fn composed_operators(response: &plateforce_analysis::AnalysisResponse) -> Vec<String> {
    let mut ids: Vec<String> = response
        .bound_methods
        .iter()
        .filter(|bound| bound.method_id.starts_with("onset.op."))
        .map(|bound| bound.method_id.clone())
        .collect();
    ids.sort();
    ids
}

#[test]
fn a_bare_request_composes_the_weighing_epoch_end_floor_and_not_the_deprecated_one() {
    let response = run(&trial(), &bare_request()).expect("a bare request runs");
    let operators = composed_operators(&response);
    println!("{} operators composed: {operators:?}", operators.len());

    assert!(
        operators.contains(&"onset.op.search_floor_at_weighing_epoch_end".to_string()),
        "the floor a bare request uses is the weighing epoch's end: {operators:?}"
    );
    assert!(
        !operators.contains(&"onset.op.search_floor".to_string()),
        "the deprecated fixed floor runs only when a caller states one: {operators:?}"
    );
}

/// The registry ties this operator's absence to failure rising from 0.8 percent to 14.9
/// percent, so whether it runs unasked is worth a test rather than a reading of the list.
#[test]
fn the_persistence_operator_runs_when_nobody_asked_for_it() {
    let response = run(&trial(), &bare_request()).expect("a bare request runs");
    let persistence = response
        .bound_methods
        .iter()
        .find(|bound| bound.method_id == "onset.op.persistence")
        .expect("the persistence operator is composed on a bare request");

    assert!(
        persistence
            .assumed_parameters
            .contains(&"span_ms".to_string()),
        "its span is the rule's own, so it is recorded assumed: {:?}",
        persistence.bound_parameters
    );
}

/// Every operator this build composes without being asked is a registry entry, and the
/// verdict on that entry is what a surface owes the user. Reported with its denominator so a
/// pattern that matches nothing cannot read as a build that composes nothing.
#[test]
fn every_operator_composed_unasked_carries_a_verdict_a_surface_can_act_on() {
    let registry = registry();
    let response = run(&trial(), &bare_request()).expect("a bare request runs");
    let operators = composed_operators(&response);

    assert!(
        !operators.is_empty(),
        "a bare request composes no operator at all, so this test measures nothing"
    );

    let mut unfiled = Vec::new();
    let mut owed_a_display = Vec::new();
    for id in &operators {
        match registry.methods.get(id) {
            None => unfiled.push(id.clone()),
            Some(method) => {
                let surfacing = method.gui.as_ref().map(|gui| gui.surfacing);
                println!("  {id:48} {:?} {surfacing:?}", method.status);
                if surfacing == Some(Surfacing::DefaultAndShow)
                    || surfacing == Some(Surfacing::ForceADecision)
                {
                    owed_a_display.push(id.clone());
                }
            }
        }
    }

    assert!(
        unfiled.is_empty(),
        "{} of {} composed operators carry no registry entry: {unfiled:?}",
        unfiled.len(),
        operators.len()
    );

    // Not a failure. The count is the handoff to whichever surface renders it, and it goes to
    // zero when a caller resolves the choice rather than when the software stops making it.
    println!(
        "{} of {} composed operators carry a verdict the interface owes a rendering: {owed_a_display:?}",
        owed_a_display.len(),
        operators.len()
    );
}
