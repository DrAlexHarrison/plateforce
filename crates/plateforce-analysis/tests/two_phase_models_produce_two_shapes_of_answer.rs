//! What the phase family has to hold once every entry in `phase.toml` either computes or
//! names its barrier.
//!
//! Every other construct in this build produces a number, and the choice of rule moves it.
//! `phase_model` produces a partition, and the choice of rule changes how many parts there
//! are. So the property here is not that two models give two values: it is that they give
//! two different sets of keys, and that a reader holding a result can say which model
//! produced the phases in front of them without being told.
//!
//! The loaded-lift boundary is the other half. Three rules report one key and the registry
//! records 24 percent on mean force between two of them, which is larger than any training
//! effect the number is used to detect. What separates them here is measured rather than
//! asserted.

use std::collections::{BTreeMap, BTreeSet};

use plateforce_analysis::{run, AnalysisRequest, AnalysisResponse, MethodChoice, WeighingChoice};
use plateforce_core::{read_trial_from_path, Trial};

mod common;

const FIXTURE_ROOT: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../plateforce-conformance/fixtures"
);

/// The founding corpus samples at 1200 Hz. Reading these traces at 1000 corrupts every
/// velocity, displacement and boundary measured across one by 20 percent.
const CORPUS_SAMPLE_RATE_HZ: f64 = 1200.0;

/// The one committed trial that returns to the plate. Five of the six were trimmed before
/// the athlete came back down, so anything reading past takeoff has a denominator of one,
/// and this file says so where it reads past takeoff and where it does not.
fn subject01_trial1() -> Trial {
    let path = format!("{FIXTURE_ROOT}/subject01_trial1.force.txt");
    let (trial, _) = read_trial_from_path(&path, '\t', 0, CORPUS_SAMPLE_RATE_HZ)
        .unwrap_or_else(|error| panic!("{path} did not read: {error}"));
    trial
}

fn base() -> AnalysisRequest {
    common::prepared(AnalysisRequest {
        weighing: WeighingChoice {
            method_id: "bwepoch.fixed_window".into(),
            parameters: BTreeMap::from([("duration".to_string(), 1.0)]),
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

fn naming(pairs: &[(&str, &str)]) -> AnalysisRequest {
    let mut request = base();
    for (construct, method_id) in pairs {
        request.derived.insert(
            (*construct).to_string(),
            MethodChoice {
                method_id: (*method_id).to_string(),
                ..Default::default()
            },
        );
    }
    // After the slots are named, not before: a choice inserted into a prepared request carries
    // its own empty declared table and would reach a rule reading nothing.
    common::prepared(request)
}

fn value(response: &AnalysisResponse, key: &str) -> Option<f64> {
    response
        .metrics
        .iter()
        .find(|metric| metric.key == key)
        .and_then(|metric| metric.value)
}

/// Every key this rule reported a value under, which is the shape of its answer.
fn keys_reported_by(response: &AnalysisResponse, method_id: &str) -> BTreeSet<String> {
    response
        .metrics
        .iter()
        .filter(|metric| metric.computed_by.as_deref() == Some(method_id))
        .filter(|metric| metric.value.is_some())
        .map(|metric| metric.key.clone())
        .collect()
}

/// The four phase models this build can run on a countermovement trace, and what each
/// publishes.
const PHASE_MODELS: &[&str] = &[
    "phase.model.unweighting_single.mcmahon2018",
    "phase.model.unloading_yielding_split.harry2020",
    "vocab.downward_upward",
    "phase.model.squat_jump_distinct",
];

/// The whole claim of the construct, in one assertion: no two models publish the same set of
/// keys, so a result carries the shape of the model that produced it and not only its values.
///
/// This is what makes `phase_model` different from every other construct here. Two braking
/// rules give one key and two numbers, and a reader who lost the record cannot tell them
/// apart. Two phase models give two different sets of keys, and a reader who lost the record
/// still can.
#[test]
fn no_two_phase_models_publish_the_same_set_of_keys() {
    let trial = subject01_trial1();
    let mut published: Vec<(&str, BTreeSet<String>)> = Vec::new();
    for model in PHASE_MODELS {
        let response = run(&trial, &naming(&[("phase_model", model)])).expect("well formed");
        let keys = keys_reported_by(&response, model);
        assert!(
            !keys.is_empty(),
            "{model} published nothing, so this guard is comparing two empty sets"
        );
        println!("{model}: {} keys, {keys:?}", keys.len());
        published.push((model, keys));
    }

    for (index, (model, keys)) in published.iter().enumerate() {
        for (other_model, other_keys) in published.iter().skip(index + 1) {
            assert_ne!(
                keys, other_keys,
                "{model} and {other_model} publish the same keys, so a reader holding one \
                 result cannot tell which model produced it"
            );
        }
    }

    // The count is the disagreement, and it is stated as a number rather than left implicit
    // in the sets above: one model names two boundaries of the countermovement and the other
    // names five.
    let single = &published[0].1;
    let split = &published[1].1;
    assert_eq!(
        single.len(),
        2,
        "the single-phase model published {single:?}"
    );
    assert_eq!(split.len(), 5, "the split model published {split:?}");
    assert!(
        single.is_disjoint(split),
        "the two countermovement models share a key: {:?}",
        single.intersection(split).collect::<Vec<_>>()
    );
}

/// The three loaded-lift end rules on one recording, and the order the registry predicts.
///
/// Net force falling through zero is the instant after which the object's motion is pure
/// momentum, so it necessarily precedes both the top of the movement and the end of contact.
/// The two later rules land at the end of contact on an unloaded jump, which is the registry's
/// own account of why the distinction vanishes there: the athlete leaves the ground rather
/// than decelerating the load.
#[test]
fn the_net_force_rule_ends_the_lift_before_the_other_two_and_the_gap_is_the_deceleration() {
    let trial = subject01_trial1();
    const KEY: &str = "lifting_phase_end_seconds";
    let mut placed: BTreeMap<&str, f64> = BTreeMap::new();
    for rule in [
        "phase.lift.end.net_force_zero",
        "phase.lift.end.peak_displacement.lake2012_PD",
        "phase.lift.end.absolute_force_zero.frost2008",
    ] {
        let response = run(&trial, &naming(&[("lifting_phase_end", rule)])).expect("well formed");
        let seconds = value(&response, KEY).unwrap_or_else(|| panic!("{rule} placed no lift end"));
        // One key, three rules. What tells them apart on the result is `computed_by`, so a
        // reader comparing two lifts is comparing the same quantity under two named rules.
        let metric = response
            .metrics
            .iter()
            .find(|metric| metric.key == KEY)
            .expect("the key is reported");
        assert_eq!(metric.computed_by.as_deref(), Some(rule));
        placed.insert(rule, seconds);
    }

    let net = placed["phase.lift.end.net_force_zero"];
    let displacement = placed["phase.lift.end.peak_displacement.lake2012_PD"];
    let absolute = placed["phase.lift.end.absolute_force_zero.frost2008"];
    let takeoff = value(&run(&trial, &base()).unwrap(), "takeoff_time_seconds").unwrap();
    println!(
        "net force {net:.4} s, peak displacement {displacement:.4} s, absolute zero \
         {absolute:.4} s, takeoff {takeoff:.4} s"
    );
    // The interval between the two rules is the deceleration the lifter applies before the
    // movement ends, which on an unloaded jump is the stretch between peak velocity and
    // leaving the ground. Printed in milliseconds because that is the scale it lives at, and
    // because the lab measured the same interval independently at 17.5 to 20.0 ms.
    println!(
        "net force to the top of the movement: {:.1} ms",
        1000.0 * (displacement - net)
    );
    println!(
        "the three rules span {:.1} ms, and takeoff sits inside that span",
        1000.0 * (absolute - net)
    );

    assert!(
        net < displacement,
        "net force at {net:.4} s did not precede the top of the movement at {displacement:.4} s"
    );
    assert!(
        net < absolute,
        "net force at {net:.4} s did not precede the end of contact at {absolute:.4} s"
    );
    // Stated as a size as well as an order. Three rules one sample apart would satisfy the
    // ordering while telling a reader the three names are interchangeable, and the registry
    // records 24 percent on mean force between two of them.
    assert!(
        displacement - net > 0.01,
        "the two rules landed {:.4} s apart, so this trace does not tell their names apart",
        displacement - net
    );
}

/// A rule whose input is a person runs and asks, rather than being filed as unreachable.
///
/// Both halves are asserted: an unstated instant is refused by name, and a stated one is
/// honoured to the sample. A guard checking only the refusal would pass on a rule that could
/// never place anything, which is the shape this entry was previously mistaken for.
#[test]
fn the_hand_placed_lift_start_refuses_an_unstated_instant_and_honours_a_stated_one() {
    let trial = subject01_trial1();
    const ID: &str = "phase.lift.start.visual_inspection.dead_start";
    const KEY: &str = "lifting_phase_start_seconds";

    let unstated = run(&trial, &naming(&[("lifting_phase_start", ID)])).expect("well formed");
    assert!(
        value(&unstated, KEY).is_none(),
        "a lift start was placed with nobody having placed it"
    );
    let declined = unstated
        .refusals
        .iter()
        .find(|rule| rule.method_id == ID)
        .expect("the rule declined");
    println!("unstated: {}", declined.refusal);

    let mut stated = naming(&[("lifting_phase_start", ID)]);
    let chosen = 1500usize;
    stated
        .derived
        .get_mut("lifting_phase_start")
        .unwrap()
        .manual_index = Some(chosen);
    let response = run(&trial, &stated).expect("well formed");
    let seconds = value(&response, KEY).expect("the stated instant was placed");
    println!("stated sample {chosen} placed at {seconds:.4} s");
    assert_eq!(
        seconds,
        trial.time_at(chosen),
        "the instant a reader placed was moved"
    );

    // Two readers who placed it differently have to produce two different results, or the
    // record says the same thing about two different analyses.
    let mut elsewhere = stated.clone();
    elsewhere
        .derived
        .get_mut("lifting_phase_start")
        .unwrap()
        .manual_index = Some(chosen + 600);
    let moved = value(&run(&trial, &elsewhere).expect("well formed"), KEY)
        .expect("the second instant was placed");
    assert_ne!(
        seconds, moved,
        "moving the hand-placed instant moved no number"
    );
}

/// The four analysis windows this build can place, on the recording that holds a landing.
///
/// Every extremum and every impulse in the registry is taken over a window, so two peaks are
/// comparable only when both windows are named. These four answer four different questions and
/// the guard holds them to being four different windows.
#[test]
fn the_four_analysis_windows_are_four_different_stretches_of_one_recording() {
    let trial = subject01_trial1();
    let mut spans: Vec<(&str, f64, f64)> = Vec::new();
    for rule in [
        "window_end.takeoff.detected",
        "window_end.fixed_duration.isometric",
        "window_end.force_dropoff_from_running_max",
        "phase.window.positive_impulse.net_force_positive",
    ] {
        let response = run(&trial, &naming(&[("analysis_window", rule)])).expect("well formed");
        let start = value(&response, "analysis_window_start_seconds")
            .unwrap_or_else(|| panic!("{rule} placed no window start"));
        let end = value(&response, "analysis_window_end_seconds")
            .unwrap_or_else(|| panic!("{rule} placed no window end"));
        println!(
            "{rule}: {start:.4} s to {end:.4} s, {:.4} s long",
            end - start
        );
        assert!(end > start, "{rule} placed a window of no length");
        spans.push((rule, start, end));
    }

    for (index, (rule, start, end)) in spans.iter().enumerate() {
        for (other, other_start, other_end) in spans.iter().skip(index + 1) {
            assert!(
                (start - other_start).abs() > f64::EPSILON
                    || (end - other_end).abs() > f64::EPSILON,
                "{rule} and {other} placed the same window, so the choice between them moves \
                 nothing"
            );
        }
    }
}
