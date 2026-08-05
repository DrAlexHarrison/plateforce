//! A reader who looks up the entry a value was recorded against finds the name there.
//!
//! The record puts each value on the row of the entry that owns the choice, which is what
//! `bound_with_operators` is for. That is half the promise. The other half is that the entry,
//! looked up in the registry, says something about the name the record used, because a value
//! sitting on a row whose registry text never mentions it leaves a reader exactly where an
//! unrecorded choice would: holding a number and no way to find out what it is.
//!
//! Three ways a name is findable, and the second is why the count here is not the count a
//! naive sweep returns:
//!
//! 1. The entry declares it as a parameter, which is the ordinary case.
//! 2. The value is itself a registry id, so the name is a dimension label and the value is the
//!    pointer. The four `integration_*` names are this: `jumpheight.takeoff.impulse_momentum`
//!    records `integration_anchor = integration.anchor.single_point`, and the entry a reader
//!    wants is the one in the value. Counting these as failures reports 14 where 2 is the
//!    answer.
//! 3. The entry's own prose names it. This is what a number read off the recording gets: a
//!    rule may report where a landmark landed, and the caller cannot state it, so declaring it
//!    a parameter would say the opposite of what is true.
//!
//! Both controls are asserted rather than assumed, because a sweep that found no failures
//! because it read no values reads exactly like a clean build.

use std::collections::{BTreeMap, BTreeSet};

use plateforce_analysis::{run, AnalysisRequest, AnalysisResponse, MethodChoice, WeighingChoice};
use plateforce_core::Trial;
use plateforce_registry::Registry;

const SAMPLE_RATE_HZ: f64 = 1200.0;

fn registry() -> Registry {
    Registry::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../registry"))
        .expect("the shipped registry loads")
}

/// Quiet stance, an unweighting dip, a push, flight, then a landing, so every landmark is
/// placed and the rules that compose operators onto a threshold all run.
fn a_jump_that_lands() -> Trial {
    let mut force = vec![600.0; 1200];
    for (index, sample) in force.iter_mut().enumerate() {
        *sample += ((index % 17) as f64 - 8.0) * 0.4;
    }
    force.extend((0..360).map(|index| 600.0 - 300.0 * (index as f64 / 360.0)));
    force.extend((0..360).map(|index| 300.0 + 1200.0 * (index as f64 / 360.0)));
    force.extend(std::iter::repeat_n(0.0, 600));
    force.extend(std::iter::repeat_n(2400.0, 240));
    force.extend(std::iter::repeat_n(600.0, 600));
    Trial::new(force, SAMPLE_RATE_HZ).unwrap()
}

/// The onset rule that bounds its search above, because the value entry 8 of the queue was
/// written about is the instant that bound landed on.
fn a_request_whose_onset_bounds_its_search() -> AnalysisRequest {
    AnalysisRequest {
        weighing: WeighingChoice {
            method_id: "bwepoch.fixed_window".into(),
            parameters: BTreeMap::from([("duration".to_string(), 1.0)]),
            ..Default::default()
        },
        onset: MethodChoice {
            method_id: "onset.threshold.last_within_band".into(),
            ..Default::default()
        },
        takeoff: MethodChoice {
            method_id: "takeoff.threshold.absolute_force".into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

struct Recorded {
    entry: String,
    name: String,
    value: String,
}

fn recorded(response: &AnalysisResponse) -> Vec<Recorded> {
    response
        .bound_methods
        .iter()
        .flat_map(|bound| {
            bound.bound_parameters.iter().map(|(name, value)| Recorded {
                entry: bound.method_id.clone(),
                name: name.clone(),
                value: value.clone(),
            })
        })
        .collect()
}

/// How a name is findable on the entry it was recorded against, or `None` when it is not.
fn findable(registry: &Registry, ids: &BTreeSet<&str>, item: &Recorded) -> Option<&'static str> {
    let entry = registry.methods.get(&item.entry)?;
    if entry.parameters.iter().any(|p| p.name == item.name) {
        return Some("declared as a parameter");
    }
    if ids.contains(item.value.as_str()) {
        return Some("the value is a registry id");
    }
    let prose = [entry.rule.as_str(), entry.title.as_str()]
        .into_iter()
        .chain(entry.parameters.iter().filter_map(|p| p.notes.as_deref()))
        .any(|text| names_the_word(text, &item.name));
    prose.then_some("named in the entry's own text")
}

/// Whether prose names this value, as a whole word.
///
/// `contains` would do for the two names this reaches today, which are long and distinctive.
/// It would also let a name like `k` pass against any sentence with a `k` in it, which is a
/// route to findable that finds nothing, and the day that name appears here is the day nobody
/// is looking.
fn names_the_word(text: &str, name: &str) -> bool {
    text.match_indices(name).any(|(at, _)| {
        let before = text[..at].chars().next_back();
        let after = text[at + name.len()..].chars().next();
        let outside =
            |character: Option<char>| character.is_none_or(|c| !c.is_alphanumeric() && c != '_');
        outside(before) && outside(after)
    })
}

#[test]
fn every_value_the_record_places_on_an_entry_is_findable_on_that_entry() {
    let registry = registry();
    let ids: BTreeSet<&str> = registry.methods.keys().map(String::as_str).collect();
    let response = run(
        &a_jump_that_lands(),
        &a_request_whose_onset_bounds_its_search(),
    )
    .expect("the request places every landmark");
    let items = recorded(&response);

    // A sweep over nothing passes every assertion below it. The denominator is the guard on
    // the guard, and 15 is well under what a full run records, so it does not go stale the
    // week somebody adds a parameter.
    assert!(
        items.len() >= 15,
        "read {} recorded values, which is too few to have exercised the record",
        items.len()
    );

    // Control for reading 1: an entry that declares no parameter at all must not be able to
    // satisfy the check by that route. Both other routes have to be reachable too, or a green
    // here says only that the first route works.
    let by_route: BTreeMap<&str, usize> = items
        .iter()
        .filter_map(|item| findable(&registry, &ids, item))
        .fold(BTreeMap::new(), |mut counts, route| {
            *counts.entry(route).or_default() += 1;
            counts
        });
    assert!(
        by_route.len() >= 3,
        "only {} of the three routes to findable were exercised: {by_route:?}",
        by_route.len()
    );

    let lost: Vec<String> = items
        .iter()
        .filter(|item| findable(&registry, &ids, item).is_none())
        .map(|item| format!("{} <- {} = {}", item.entry, item.name, item.value))
        .collect();
    assert!(
        lost.is_empty(),
        "{} of {} recorded values sit on an entry that says nothing about the name they were \
         recorded under, so a reader who looks the entry up finds the value and no account of \
         it: {lost:#?}",
        lost.len(),
        items.len()
    );
}

/// The two the queue was written about, named rather than left to the sweep above.
///
/// The sweep passes on a build that stopped recording either one, because a value nobody
/// records cannot be recorded against the wrong entry. This asserts the pair is still
/// produced, and produced on the operator that owns the choice rather than on the threshold
/// rule that composed it.
#[test]
fn the_two_instants_read_off_the_trace_are_recorded_on_the_entries_that_own_them() {
    let response = run(
        &a_jump_that_lands(),
        &a_request_whose_onset_bounds_its_search(),
    )
    .expect("the request places every landmark");
    let items = recorded(&response);
    for (entry, name) in [
        ("onset.op.search_upper_bound", "search_bound_seconds"),
        ("bwepoch.fixed_window", "start_seconds"),
    ] {
        let found = items
            .iter()
            .find(|item| item.entry == entry && item.name == name);
        assert!(
            found.is_some(),
            "{name} was recorded against {:?}, not against {entry}",
            items
                .iter()
                .filter(|item| item.name == name)
                .map(|item| item.entry.as_str())
                .collect::<Vec<_>>()
        );
    }
}
