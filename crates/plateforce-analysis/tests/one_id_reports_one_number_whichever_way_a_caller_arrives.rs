//! A quantity whose declaration names a registry entry reports one number under that entry's
//! id, whether the caller named the rule or left the software to reach it.
//!
//! `jumpheight.takeoff.flight_time` is where the two routes can part: the entry publishes four
//! gravities and declares 9.81, while the request type fills in 9.80665, and a second
//! computation of the height under the entry's name would report the two in the ratio
//! 9.81 / 9.80665. One id meaning two things depending on how the caller phrased the request
//! is a number that cannot be looked up and reproduced.

use std::collections::BTreeMap;

use plateforce_analysis::binding::Dispatch;
use plateforce_analysis::{run, AnalysisRequest, AnalysisResponse, MethodChoice, WeighingChoice};
use plateforce_core::provenance::ParameterSource;
use plateforce_core::Trial;

mod common;

const SAMPLE_RATE_HZ: f64 = 1200.0;
const FLIGHT_SAMPLES: usize = 811;
const FLIGHT_KEY: &str = "jump_height_from_flight_time_meters";
const FLIGHT_TIME_RULE: &str = "jumpheight.takeoff.flight_time";
const GRAVITY: &str = "gravity";

/// A countermovement jump that leaves the plate and lands back on it, so every rule below has
/// the three landmarks and the return it needs and none of them declines for want of one.
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
        ..Default::default()
    })
}

fn naming(construct: &str, method_id: &str) -> AnalysisRequest {
    let mut request = base();
    request.derived.insert(
        construct.to_string(),
        MethodChoice {
            method_id: method_id.to_string(),
            ..Default::default()
        },
    );
    // After the slot is named, not before: a choice inserted into a prepared request carries its
    // own empty declared table and would reach a rule reading nothing.
    common::prepared(request)
}

fn metric<'a>(
    response: &'a AnalysisResponse,
    key: &str,
) -> Option<&'a plateforce_analysis::Metric> {
    response.metrics.iter().find(|metric| metric.key == key)
}

fn bound<'a>(
    response: &'a AnalysisResponse,
    method_id: &str,
) -> Option<&'a plateforce_analysis::BoundMethod> {
    response
        .bound_methods
        .iter()
        .find(|row| row.method_id == method_id)
}

/// Every quantity the spine reports whose declaration names an entry this build runs a rule
/// for, paired with the construct that rule fills.
///
/// Read off the binding table rather than listed here, so a rule added under one of these
/// constructs is covered without an edit, and a quantity that stops naming an entry drops out
/// rather than being asserted against a rule that no longer produces it.
fn spine_quantities_backed_by_a_rule() -> Vec<(&'static str, &'static str, &'static str)> {
    let mut pairs = Vec::new();
    for quantity in plateforce_analysis::response::SPINE_QUANTITIES {
        let Some(id) = quantity.computed_by else {
            continue;
        };
        let Some(binding) = plateforce_analysis::BINDINGS
            .iter()
            .find(|binding| binding.id == id && matches!(binding.dispatch, Dispatch::Derived(_)))
        else {
            continue;
        };
        pairs.push((quantity.key, binding.construct, binding.id));
    }
    pairs
}

/// Naming a rule is a statement about which arithmetic to run, never about what that
/// arithmetic should produce, so a number that moves when a caller names the rule that was
/// already producing it is two methods under one id.
///
/// Every such quantity, because a guard written against a single case would pass on the day a
/// second entry started publishing a constant of its own.
#[test]
fn naming_the_rule_that_already_produced_a_number_does_not_move_it() {
    let trial = a_jump_that_lands();
    let quantities = spine_quantities_backed_by_a_rule();
    let mut checked = 0usize;

    for (key, construct, method_id) in &quantities {
        let unnamed = run(&trial, &base()).expect("the request is well formed");
        let named = run(&trial, &naming(construct, method_id)).expect("the request is well formed");

        let before = metric(&unnamed, key).and_then(|metric| metric.value);
        let after = metric(&named, key).and_then(|metric| metric.value);
        println!("{key}: unnamed {before:?}, naming {method_id} {after:?}");
        assert!(
            before.is_some(),
            "{key} is absent before the rule is named, so this pair proves nothing"
        );
        assert_eq!(
            before, after,
            "{key} moved when {method_id} was named, so one id carries two methods"
        );

        // And the chain, on every quantity. Naming a rule states which arithmetic to run and
        // says nothing about what that arithmetic reads, so a chain that moves when the
        // caller names the rule that was already producing the number is one id carrying two
        // accounts of itself. The single-key version below is satisfied by any quantity the
        // spine computes under a name whose rule it never runs.
        let chain_before = metric(&unnamed, key).map(|metric| &metric.contributing_method_ids);
        let chain_after = metric(&named, key).map(|metric| &metric.contributing_method_ids);
        assert_eq!(
            chain_before, chain_after,
            "{key} named one set of rules when the software reached {method_id} and another when the caller did"
        );
        assert!(
            chain_before.is_some_and(|chain| !chain.is_empty()),
            "{key} carries an empty chain, so the comparison above compared nothing"
        );
        checked += 1;
    }

    println!(
        "{checked} of {} spine quantities name a rule this build runs",
        quantities.len()
    );
    // The population this was written against. Seven quantities name an entry with a rule
    // behind it, and a guard whose subject shrank below that would pass by having less to
    // read.
    assert!(
        checked >= 7,
        "only {checked} spine quantities were reached, so the subject has shrunk"
    );
    let interval_and_flight = ["time_to_takeoff_seconds", "flight_time_seconds"];
    for key in interval_and_flight {
        assert!(
            quantities.iter().any(|(reached, _, _)| *reached == key),
            "{key} is not in the population, so this guard cannot see the case it was written for"
        );
    }
}

/// And the whole record agrees, not only the number. Two results that carry one number under
/// two different chains are still two answers to the question a reader asks of this tool,
/// which is what produced it.
#[test]
fn the_flight_time_height_carries_one_record_whichever_way_it_was_reached() {
    let trial = a_jump_that_lands();
    let unnamed = run(&trial, &base()).expect("the request is well formed");
    let named = run(
        &trial,
        &naming("jump_height.takeoff_frame", FLIGHT_TIME_RULE),
    )
    .expect("the request is well formed");

    let reached_by_default = metric(&unnamed, FLIGHT_KEY).expect("the software reached the rule");
    let reached_by_name = metric(&named, FLIGHT_KEY).expect("the caller named the rule");
    println!(
        "default {:?} over {:?}\nnamed   {:?} over {:?}",
        reached_by_default.value,
        reached_by_default.contributing_method_ids,
        reached_by_name.value,
        reached_by_name.contributing_method_ids
    );
    assert_eq!(reached_by_default.value, reached_by_name.value);
    assert_eq!(
        reached_by_default.contributing_method_ids, reached_by_name.contributing_method_ids,
        "one id named two different sets of rules behind one number"
    );
    assert_eq!(
        reached_by_default.computed_by.as_deref(),
        Some(FLIGHT_TIME_RULE)
    );

    // And the chain names what conditioned the signal, on both routes. Comparing the two
    // chains against each other cannot show this: one function builds both, so a change that
    // dropped the conditioning rules would drop them from both and the comparison would still
    // hold. The rules that ran are read off the result instead.
    let conditioning_that_ran: Vec<&str> = unnamed
        .bound_methods
        .iter()
        .filter(|row| {
            plateforce_analysis::BINDINGS.iter().any(|binding| {
                binding.id == row.method_id && matches!(binding.dispatch, Dispatch::Conditioning(_))
            })
        })
        .map(|row| row.method_id.as_str())
        .collect();
    assert!(
        !conditioning_that_ran.is_empty(),
        "no conditioning rule ran, so this cannot show that the chain names one"
    );
    for chain in [
        &reached_by_default.contributing_method_ids,
        &reached_by_name.contributing_method_ids,
    ] {
        for id in &conditioning_that_ran {
            let named_at = chain.iter().position(|step| step == id);
            assert!(
                named_at.is_some(),
                "the number was measured on the series {id} produced and its chain does not name it: {chain:?}"
            );
            let landmark_at = chain
                .iter()
                .position(|step| step == "takeoff.threshold.absolute_force");
            assert!(
                named_at < landmark_at,
                "{id} conditioned the signal the landmarks were placed on and is named after them: {chain:?}"
            );
        }
    }

    // The rule leaves a record either way. A result naming a registry entry and carrying
    // nothing about the gravity that produced the number is a citation with no method behind
    // it.
    let by_default = bound(&unnamed, FLIGHT_TIME_RULE).expect("the defaulted rule left a record");
    let by_name = bound(&named, FLIGHT_TIME_RULE).expect("the named rule left a record");
    assert_eq!(by_default.bound_parameters, by_name.bound_parameters);
    assert_eq!(by_default.parameter_sources, by_name.parameter_sources);
}

/// The gravity is on the record with where it came from, so a reader can tell the entry's
/// published value from one somebody chose.
#[test]
fn the_gravity_behind_the_height_is_recorded_and_says_whether_anybody_chose_it() {
    let trial = a_jump_that_lands();

    let assumed = run(&trial, &base()).expect("the request is well formed");
    let row = bound(&assumed, FLIGHT_TIME_RULE).expect("the rule left a record");
    let shown = row
        .bound_parameters
        .iter()
        .find(|(name, _)| name == GRAVITY)
        .map(|(_, value)| value.clone())
        .expect("the record names the gravity the number was computed at");
    println!(
        "nobody chose one: gravity {shown}, source {:?}",
        row.parameter_sources.get(GRAVITY)
    );
    assert_eq!(
        row.parameter_sources.get(GRAVITY),
        Some(&ParameterSource::Assumed),
        "a value nobody stated is recorded as one somebody did"
    );

    // A value stated on the rule is the caller answering this entry's own question.
    let mut on_the_rule = naming("jump_height.takeoff_frame", FLIGHT_TIME_RULE);
    on_the_rule
        .derived
        .get_mut("jump_height.takeoff_frame")
        .unwrap()
        .parameters
        .insert(GRAVITY.to_string(), 9.79);
    let stated = run(&trial, &on_the_rule).expect("the request is well formed");
    let row = bound(&stated, FLIGHT_TIME_RULE).expect("the rule left a record");
    assert_eq!(
        row.parameter_sources.get(GRAVITY),
        Some(&ParameterSource::Stated)
    );
}

/// A gravity chosen for the whole analysis moves the height, because the height is
/// `g t^2 / 8` and nothing else about the trace changed.
///
/// The property the sweep rests on. `spread.rs` varies gravity as an axis, and a rule that
/// answered with its entry's published constant regardless would report a spread of zero over
/// a knob that had moved. The request's gravity is honoured where the record can say somebody
/// chose it: left as the constant the request type fills in for everybody, the entry's value
/// stands.
#[test]
fn a_gravity_chosen_for_the_analysis_moves_the_height_and_the_filled_in_one_does_not() {
    let trial = a_jump_that_lands();
    let seconds = FLIGHT_SAMPLES as f64 / SAMPLE_RATE_HZ;

    let mut chosen = base();
    chosen.gravity_meters_per_second_squared = 9.79;
    chosen.gravity_source = ParameterSource::Stated;
    let moved = metric(
        &run(&trial, &chosen).expect("the request is well formed"),
        FLIGHT_KEY,
    )
    .and_then(|metric| metric.value)
    .expect("a height");

    // The same number the request carries, and nobody claiming to have chosen it. The entry's
    // published default stands, because falling back to a value no entry declares would be a
    // silent default wearing a declared one's paperwork.
    let mut filled_in = base();
    filled_in.gravity_meters_per_second_squared = 9.79;
    let untouched = metric(
        &run(&trial, &filled_in).expect("the request is well formed"),
        FLIGHT_KEY,
    )
    .and_then(|metric| metric.value)
    .expect("a height");

    println!("chosen 9.79 gives {moved:.6} m, the same number filled in gives {untouched:.6} m");
    assert!(
        (moved - 9.79 * seconds * seconds / 8.0).abs() < 1e-9,
        "a gravity somebody chose did not reach the projectile equation"
    );
    assert!(
        (untouched - 9.81 * seconds * seconds / 8.0).abs() < 1e-9,
        "a gravity nobody chose displaced the value the entry declares"
    );
    assert_ne!(
        moved, untouched,
        "the two claims produced one number, so the record cannot be telling them apart"
    );
}
