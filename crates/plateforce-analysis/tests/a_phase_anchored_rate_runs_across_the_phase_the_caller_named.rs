//! The two rate rules that run across a phase run across the one the caller named, and say so.
//!
//! Each is checked by multiplying its own arithmetic back out against the samples between the
//! two boundaries that phase names, read off the same result. An exact identity rather than a
//! tolerance, so a rule reading a different stretch of the same trace fails here even where the
//! number it returns is the right order of magnitude.
//!
//! The three phases are the control for each other. A rule ignoring the name would return one
//! number three times, and a check that only asserted each identity against the interval the
//! rule chose would agree with it every time.
//!
//! The implementing paper computes this rate for the unloading, eccentric yielding and eccentric
//! braking phases and not for the concentric phase, so the phase is required and refused by name
//! when unstated. That refusal is measured here too, because a required value nothing refuses is
//! a default under another word.

use std::collections::BTreeMap;

use plateforce_analysis::{run, AnalysisRequest, AnalysisResponse, MethodChoice, WeighingChoice};
use plateforce_core::Trial;

mod common;

const RATE_KEY: &str = "rate_of_force_development_newtons_per_second";
const SECANT: &str = "rfd.phase_endpoint_secant.harry";
const OVER_DURATION: &str = "rfd.mean_force_over_duration.lapuente";

/// Every phase a request can name, with the keys of the two boundaries it runs between. `None`
/// for the start is movement onset, which a response reports as a field rather than a metric.
const PHASES: &[(&str, Option<&str>, &str)] = &[
    (
        "onset_to_braking_start",
        None,
        "braking_phase_start_seconds",
    ),
    (
        "braking_start_to_propulsion_start",
        Some("braking_phase_start_seconds"),
        "propulsion_phase_start_seconds",
    ),
    (
        "propulsion_start_to_propulsion_end",
        Some("propulsion_phase_start_seconds"),
        "propulsion_phase_end_seconds",
    ),
];

fn subject01_trial1() -> Trial {
    let (trial, _) = plateforce_core::read::read_trial_from_path(
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../plateforce-conformance/fixtures/subject01_trial1.force.txt"
        ),
        '\t',
        0,
        1200.0,
    )
    .expect("the committed trial reads");
    trial
}

/// A request reaching one phase-anchored rate rule, with a rule for every construct whose
/// boundary any of the three phases runs between.
///
/// The propulsion end is the peak-velocity rule with its signal stated, because that entry
/// publishes no default. Which propulsion-end rule is bound moves the third phase's right edge
/// and nothing else here reads it, so the identity below holds against whichever is named.
fn asking(method_id: &str, options: BTreeMap<String, String>) -> AnalysisRequest {
    asking_with_propulsion_ending_at(
        method_id,
        options,
        "phase.propulsion_end.peak_com_velocity",
        BTreeMap::from([("search_signal".to_string(), "velocity_argmax".to_string())]),
    )
}

fn asking_with_propulsion_ending_at(
    method_id: &str,
    options: BTreeMap<String, String>,
    propulsion_end_id: &str,
    propulsion_end_options: BTreeMap<String, String>,
) -> AnalysisRequest {
    let mut derived = BTreeMap::new();
    derived.insert(
        "analysis_window".to_string(),
        MethodChoice {
            method_id: "window_end.takeoff.detected".into(),
            ..Default::default()
        },
    );
    derived.insert(
        "braking_phase_start".to_string(),
        MethodChoice {
            method_id: "phase.braking_start.zero_net_force".into(),
            ..Default::default()
        },
    );
    derived.insert(
        "propulsion_phase_start".to_string(),
        MethodChoice {
            method_id: "phase.propulsion_start.zero_velocity".into(),
            ..Default::default()
        },
    );
    derived.insert(
        "propulsion_phase_end".to_string(),
        MethodChoice {
            method_id: propulsion_end_id.into(),
            options: propulsion_end_options,
            ..Default::default()
        },
    );
    derived.insert(
        "rate_of_force_development".to_string(),
        MethodChoice {
            method_id: method_id.into(),
            options,
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
        ..Default::default()
    })
}

fn across(method_id: &str, phase: &str) -> AnalysisResponse {
    run(
        &subject01_trial1(),
        &asking(
            method_id,
            BTreeMap::from([("phase".to_string(), phase.to_string())]),
        ),
    )
    .expect("the analysis ran")
}

fn value(response: &AnalysisResponse, key: &str) -> Option<f64> {
    response
        .metrics
        .iter()
        .find(|metric| metric.key == key)
        .and_then(|metric| metric.value)
}

/// The two samples one phase runs between, read off the result that reported the rate.
fn interval(response: &AnalysisResponse, start_key: Option<&str>, end_key: &str) -> (usize, usize) {
    let trial = subject01_trial1();
    let rate_hz = trial.sample_rate_hz();
    let at = |seconds: f64| (seconds * rate_hz).round() as usize;
    let start = match start_key {
        Some(key) => at(value(response, key).unwrap_or_else(|| panic!("{key} was placed"))),
        None => response.onset_index.expect("onset was placed"),
    };
    let end = at(value(response, end_key).unwrap_or_else(|| panic!("{end_key} was placed")));
    (start, end)
}

#[test]
fn the_secant_is_the_force_between_the_two_boundaries_of_the_phase_that_was_named() {
    let trial = subject01_trial1();
    let force = trial.force();
    let interval_seconds = trial.sample_interval_seconds();
    let mut rates: Vec<f64> = Vec::new();

    for (phase, start_key, end_key) in PHASES {
        let response = across(SECANT, phase);
        let rate = value(&response, RATE_KEY)
            .unwrap_or_else(|| panic!("{SECANT} produced no rate across {phase}"));
        let (start, end) = interval(&response, *start_key, end_key);
        let duration_seconds = (end - start) as f64 * interval_seconds;
        println!(
            "{phase}: samples {start} to {end}, {duration_seconds:.4} s, force {:.1} to {:.1} N, \
             {rate:.2} N/s",
            force[start], force[end]
        );
        assert!(
            (rate * duration_seconds - (force[end] - force[start])).abs() < 1e-6,
            "{SECANT} across {phase} reads {rate:.4} N/s over {duration_seconds:.4} s, which is \
             {:.4} N against the {:.4} N between the two boundaries that phase names",
            rate * duration_seconds,
            force[end] - force[start]
        );
        rates.push(rate);
    }

    // Three identities against three intervals say nothing on their own if the rule returned
    // one number and the intervals happened to fit it. The distinctness is what makes each
    // identity a reading of the phase that was named.
    for (position, rate) in rates.iter().enumerate() {
        for other in rates.iter().skip(position + 1) {
            assert!(
                (rate - other).abs() > 1.0,
                "two phases produced the same rate, {rate:.4} N/s and {other:.4} N/s, so this \
                 run says nothing about which phase the rule read"
            );
        }
    }
}

#[test]
fn the_mean_over_a_duration_is_the_mean_of_the_phase_that_was_named() {
    let trial = subject01_trial1();
    let force = trial.force();
    let interval_seconds = trial.sample_interval_seconds();
    let mut rates: Vec<f64> = Vec::new();

    for (phase, start_key, end_key) in PHASES {
        let response = across(OVER_DURATION, phase);
        let rate = value(&response, RATE_KEY)
            .unwrap_or_else(|| panic!("{OVER_DURATION} produced no rate across {phase}"));
        let (start, end) = interval(&response, *start_key, end_key);
        let duration_seconds = (end - start) as f64 * interval_seconds;
        let mean_newtons = plateforce_core::statistics::mean(&force[start..=end])
            .expect("the stretch holds samples");
        println!("{phase}: mean {mean_newtons:.2} N over {duration_seconds:.4} s, {rate:.2} N/s");
        assert!(
            (rate * duration_seconds - mean_newtons).abs() < 1e-6,
            "{OVER_DURATION} across {phase} reads {rate:.4} over {duration_seconds:.4} s, which \
             is {:.4} N against the {mean_newtons:.4} N mean of the samples that phase names",
            rate * duration_seconds
        );
        rates.push(rate);
    }

    for (position, rate) in rates.iter().enumerate() {
        for other in rates.iter().skip(position + 1) {
            assert!(
                (rate - other).abs() > 1.0,
                "two phases produced the same value, {rate:.4} and {other:.4}, so this run says \
                 nothing about which phase the rule read"
            );
        }
    }
}

/// The value a caller stated reaches the record, so a reader of one number knows which phase
/// produced it without rerunning anything.
#[test]
fn the_phase_the_caller_named_is_on_the_record_beside_the_number() {
    for method_id in [SECANT, OVER_DURATION] {
        for (phase, _, _) in PHASES {
            let response = across(method_id, phase);
            let bound = response
                .bound_methods
                .iter()
                .find(|bound| bound.method_id == method_id)
                .unwrap_or_else(|| panic!("{method_id} is not on the record"));
            let (_, recorded) = bound
                .bound_parameters
                .iter()
                .find(|(name, _)| name == "phase")
                .unwrap_or_else(|| panic!("{method_id} recorded no phase across {phase}"));
            assert_eq!(
                recorded, phase,
                "{method_id} recorded {recorded} where the caller stated {phase}"
            );
            // Stated by the caller rather than filled in, which is the difference between a
            // record of a choice and a record of a fallback.
            assert_eq!(
                bound.parameter_sources.get("phase"),
                Some(&plateforce_core::provenance::ParameterSource::Stated),
                "{method_id} recorded the caller's phase under another source"
            );
        }
    }
}

/// An unstated phase is refused by name, and an unknown one is refused with the names this
/// rule takes.
#[test]
fn a_phase_nobody_stated_is_refused_by_name_rather_than_chosen() {
    for method_id in [SECANT, OVER_DURATION] {
        let unstated = run(&subject01_trial1(), &asking(method_id, BTreeMap::new()))
            .expect("the analysis ran");
        let refusal = unstated
            .refusals
            .iter()
            .find(|rule| rule.method_id == method_id)
            .unwrap_or_else(|| panic!("{method_id} ran without a phase"))
            .refusal
            .to_string();
        println!("{method_id}, unstated: {refusal}");
        assert!(
            refusal.contains("phase"),
            "{method_id} declined without naming what it wanted: {refusal}"
        );
        assert!(
            value(&unstated, RATE_KEY).is_none(),
            "{method_id} declined and a rate arrived anyway"
        );

        let unknown = across(method_id, "concentric");
        let refusal = unknown
            .refusals
            .iter()
            .find(|rule| rule.method_id == method_id)
            .unwrap_or_else(|| panic!("{method_id} accepted a phase it does not take"))
            .refusal
            .to_string();
        println!("{method_id}, unknown: {refusal}");
        for offered in PHASES {
            assert!(
                refusal.contains(offered.0),
                "{method_id} refused an unknown phase without offering {}: {refusal}",
                offered.0
            );
        }
    }
}

/// The entry's own claim, measured: the secant inherits the phase-boundary disagreement at both
/// ends rather than at one, so it moves with the propulsion-end school as well as with the
/// phase, and across the propulsion phase it is negative under either school.
///
/// Force at the propulsion start is above system weight and force at either candidate
/// propulsion end is at or below it, so moving the right end later moves the endpoint force
/// down and the secant further below zero. Naming this rule a rate of force development and
/// printing it without its phase reports a fall as a rise.
#[test]
fn the_secant_across_the_propulsion_phase_is_negative_under_both_propulsion_end_schools() {
    let trial = subject01_trial1();
    let force = trial.force();
    let interval_seconds = trial.sample_interval_seconds();
    let mut under_each_school: Vec<(&str, f64)> = Vec::new();

    for (school, propulsion_end_id, propulsion_end_options) in [
        (
            "peak centre of mass velocity",
            "phase.propulsion_end.peak_com_velocity",
            BTreeMap::from([("search_signal".to_string(), "velocity_argmax".to_string())]),
        ),
        ("takeoff", "phase.propulsion_end.takeoff", BTreeMap::new()),
    ] {
        let response = run(
            &trial,
            &asking_with_propulsion_ending_at(
                SECANT,
                BTreeMap::from([(
                    "phase".to_string(),
                    "propulsion_start_to_propulsion_end".to_string(),
                )]),
                propulsion_end_id,
                propulsion_end_options,
            ),
        )
        .expect("the analysis ran");
        let rate = value(&response, RATE_KEY)
            .unwrap_or_else(|| panic!("{SECANT} produced no rate under {school}"));
        let (start, end) = interval(
            &response,
            Some("propulsion_phase_start_seconds"),
            "propulsion_phase_end_seconds",
        );
        println!(
            "propulsion ended at {school}: samples {start} to {end}, force {:.1} to {:.1} N, \
             {rate:.2} N/s",
            force[start], force[end]
        );
        assert!(
            (rate * ((end - start) as f64 * interval_seconds) - (force[end] - force[start])).abs()
                < 1e-6,
            "the secant under {school} does not multiply back out to the samples it names"
        );
        assert!(
            rate < 0.0,
            "the secant across the propulsion phase read {rate:.2} N/s under {school}, where \
             force falls from above system weight to at or below it"
        );
        under_each_school.push((school, rate));
    }

    // The later end is the more negative one, which is the direction the physics fixes and the
    // opposite of what a reader expects from a longer push. A run where both schools agreed
    // would leave the entry's inherits-at-both-ends claim unread.
    let (first_school, first) = under_each_school[0];
    let (second_school, second) = under_each_school[1];
    assert!(
        second < first,
        "propulsion ended at {second_school} read {second:.2} N/s against {first:.2} N/s ended \
         at {first_school}, so moving the end later did not move the endpoint force down"
    );
}
