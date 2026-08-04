//! Which operators run when a caller states nothing, and what the registry says about each.
//!
//! `ONSET_OPERATOR_IDS` lists what this build can compose, not what it does compose, and the
//! two were read as one. Composing an operator is a choice made on the user's behalf, so the
//! set that actually runs on a bare request is pinned here rather than inferred from the list.

use std::collections::{BTreeMap, BTreeSet};

use plateforce_analysis::{run, AnalysisRequest, MethodChoice, WeighingChoice};
use plateforce_core::{Trial, STANDARD_GRAVITY_METERS_PER_SECOND_SQUARED};
use plateforce_registry::schema::{Status, Surfacing};
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
    }
}

/// Every operator the response records, whichever construct composed it.
///
/// This read `onset.op.` for as long as it existed, so the five takeoff operators the registry
/// carries were outside every assertion below. A takeoff landmark rests on a search floor, a
/// crossing selection and a residual comparison exactly as an onset landmark does, and the
/// filter that named one construct made the other three invisible rather than absent.
fn composed_operators(response: &plateforce_analysis::AnalysisResponse) -> Vec<String> {
    let mut ids: Vec<String> = response
        .bound_methods
        .iter()
        .filter(|bound| bound.method_id.contains(".op."))
        .map(|bound| bound.method_id.clone())
        .collect();
    ids.sort();
    ids
}

/// Which construct's namespace an id sits in, which is how the registry spells a rule's home.
fn construct_of(method_id: &str) -> &str {
    method_id.split('.').next().unwrap_or(method_id)
}

/// Where each construct's search may begin, asked of every construct rather than of one.
///
/// The registry carries two floor rules for onset and two for takeoff, so each landmark is a
/// silent choice between them until something says which one runs. Read from the registry
/// rather than named here: a third floor rule for either construct joins this assertion
/// without an edit, and a construct that gains its first pair is covered the day it does.
#[test]
fn every_construct_anchors_its_search_at_the_same_place_and_composes_no_deprecated_rule() {
    let registry = registry();
    let response = run(&trial(), &bare_request()).expect("a bare request runs");
    let operators = composed_operators(&response);
    println!("{} operators composed: {operators:?}", operators.len());

    let floors_offered: BTreeMap<&str, Vec<&String>> =
        registry
            .methods
            .keys()
            .filter(|id| id.contains(".op.search_floor"))
            .fold(BTreeMap::new(), |mut found, id| {
                found.entry(construct_of(id)).or_default().push(id);
                found
            });
    let with_a_choice: Vec<&&str> = floors_offered
        .iter()
        .filter(|(_, offered)| offered.len() > 1)
        .map(|(construct, _)| construct)
        .collect();
    println!("floor rules the registry offers, by construct: {floors_offered:?}");

    // The population, asked before the assertions rather than after. A registry offering one
    // floor per construct makes every line below true by having nothing to choose between.
    assert!(
        with_a_choice.len() > 1,
        "only {with_a_choice:?} has more than one floor rule to choose between, so this test \
         asserts nothing about a choice: {floors_offered:?}"
    );

    let mut anchors: BTreeMap<&str, Vec<&String>> = BTreeMap::new();
    for id in &operators {
        if id.contains(".op.search_floor") {
            anchors
                .entry(id.trim_start_matches(construct_of(id)))
                .or_default()
                .push(id);
        }
    }
    assert_eq!(
        anchors.len(),
        1,
        "two landmarks on one trial began searching from different places, so they rest on \
         different assumptions about where a jump can be: {anchors:?}"
    );

    let composed_floors: usize = anchors.values().map(Vec::len).sum();
    assert_eq!(
        composed_floors,
        with_a_choice.len(),
        "{} of the {} constructs with a floor to choose composed one: {anchors:?}",
        composed_floors,
        with_a_choice.len()
    );

    let deprecated: Vec<&String> = operators
        .iter()
        .filter(|id| {
            registry
                .methods
                .get(*id)
                .is_some_and(|method| method.status == Status::Deprecated)
        })
        .collect();
    assert!(
        deprecated.is_empty(),
        "{} of {} operators a bare request composes are deprecated, and a rule nobody asked \
         for is the software's own choice: {deprecated:?}",
        deprecated.len(),
        operators.len()
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
            .assumed_parameters()
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

    // The population, not the assertion. This test read one construct's namespace for as long
    // as it existed and reported "1 of 5" while the answer was "3 of 8", which is the shape
    // that is harder to see than an assertion that cannot fail: it was green, deliberate, and
    // blind. So the constructs the registry declares operators for are counted, and a set that
    // has stopped reaching one of them fails here rather than reporting a smaller number.
    let declared: BTreeSet<&str> = registry
        .methods
        .keys()
        .filter(|id| id.contains(".op."))
        .map(|id| construct_of(id))
        .collect();
    let reached: BTreeSet<&str> = operators.iter().map(|id| construct_of(id)).collect();
    assert_eq!(
        reached, declared,
        "the registry declares operators for {declared:?} and this test reaches {reached:?}, so \
         every number it prints is over the wrong population"
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
