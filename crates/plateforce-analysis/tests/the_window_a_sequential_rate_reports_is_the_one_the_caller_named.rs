//! What `rfd.epoch_from_onset.sequential` costs a caller, and why its window is required.
//!
//! The registry files the sequential scheme as genuinely disagreeing with the onset-anchored
//! one, and at the first window it does not: consecutive windows starting at onset make the
//! first window at width w arithmetically the overlapping scheme at epoch w. That
//! coincidence is why `window_index` is required with no default. A rule defaulting to the
//! first window would report the entry it disagrees with under this entry's name, and the
//! two would sit in the picker as a choice that moves nothing.
//!
//! The coincidence is asserted here rather than described, and the second window is the
//! control: it has to differ, or the rule is returning one window whatever it is asked for
//! and the first assertion would pass for the wrong reason.
//!
//! `force_at_epoch` is the quantity that fork cost. It is checked against the recording
//! directly, and against the rate over the same interval, because the two rules take one
//! chord and a reader reading them together is entitled to that.

use std::collections::BTreeMap;

use plateforce_analysis::{run, AnalysisRequest, AnalysisResponse, MethodChoice, WeighingChoice};
use plateforce_core::Trial;

const SAMPLE_RATE_HZ: f64 = 1200.0;
const WINDOW_MILLISECONDS: f64 = 50.0;

fn subject01_trial1() -> Trial {
    plateforce_core::read::read_trial_from_path(
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../plateforce-conformance/fixtures/subject01_trial1.force.txt"
        ),
        '\t',
        0,
        SAMPLE_RATE_HZ,
    )
    .expect("the committed trial reads")
    .0
}

/// One request, with the construct under test bound to one rule and its values stated.
fn asking(construct: &str, method_id: &str, values: &[(&str, f64)]) -> AnalysisRequest {
    AnalysisRequest {
        weighing: WeighingChoice {
            method_id: "bwepoch.fixed_window".into(),
            parameters: BTreeMap::from([("duration".to_string(), 1.0)]),
            ..Default::default()
        },
        onset: MethodChoice {
            method_id: "onset.threshold.noise_relative".into(),
            parameters: BTreeMap::from([("k".to_string(), 5.0)]),
            ..Default::default()
        },
        takeoff: MethodChoice {
            method_id: "takeoff.threshold.absolute_force".into(),
            ..Default::default()
        },
        derived: BTreeMap::from([(
            construct.to_string(),
            MethodChoice {
                method_id: method_id.into(),
                parameters: values
                    .iter()
                    .map(|(name, value)| (name.to_string(), *value))
                    .collect(),
                ..Default::default()
            },
        )]),
        ..Default::default()
    }
}

fn number(response: &AnalysisResponse, key: &str) -> Option<f64> {
    response
        .metrics
        .iter()
        .find(|metric| metric.key == key)
        .and_then(|metric| metric.value)
}

fn answered(
    trial: &Trial,
    construct: &str,
    method_id: &str,
    values: &[(&str, f64)],
    key: &str,
) -> f64 {
    let response = run(trial, &asking(construct, method_id, values)).expect("the request runs");
    number(&response, key)
        .unwrap_or_else(|| panic!("{method_id} reported no {key}, so this reads nothing"))
}

const RATE_KEY: &str = "rate_of_force_development_newtons_per_second";
const RATE_CONSTRUCT: &str = "rate_of_force_development";
const SEQUENTIAL: &str = "rfd.epoch_from_onset.sequential";
const OVERLAPPING: &str = "rfd.epoch_from_onset.overlapping";

#[test]
fn the_first_sequential_window_is_the_overlapping_scheme_at_the_same_width() {
    let trial = subject01_trial1();
    let first = answered(
        &trial,
        RATE_CONSTRUCT,
        SEQUENTIAL,
        &[("window_ms", WINDOW_MILLISECONDS), ("window_index", 1.0)],
        RATE_KEY,
    );
    let overlapping = answered(
        &trial,
        RATE_CONSTRUCT,
        OVERLAPPING,
        &[("epoch_ms", WINDOW_MILLISECONDS)],
        RATE_KEY,
    );
    assert_eq!(
        first, overlapping,
        "the first sequential window and the overlapping scheme at the same width are one \
         chord, and they read {first} and {overlapping}. Either the two rules no longer take \
         the same samples, or one of them moved its anchor"
    );

    // The control. Without it the assertion above passes for a rule that ignores the index
    // and always reports its first window, which is the shape this whole entry was refused
    // for.
    let second = answered(
        &trial,
        RATE_CONSTRUCT,
        SEQUENTIAL,
        &[("window_ms", WINDOW_MILLISECONDS), ("window_index", 2.0)],
        RATE_KEY,
    );
    assert_ne!(
        second, first,
        "the second window reported the first window's rate, {first}, so the stated index \
         moves nothing and this rule is the overlapping scheme wearing another name"
    );
}

#[test]
fn a_sequential_rate_with_no_window_named_refuses_by_that_name() {
    let trial = subject01_trial1();
    let response = run(
        &trial,
        &asking(
            RATE_CONSTRUCT,
            SEQUENTIAL,
            &[("window_ms", WINDOW_MILLISECONDS)],
        ),
    )
    .expect("the request runs, and the rule inside it declines");

    assert_eq!(
        number(&response, RATE_KEY),
        None,
        "a rate was reported for a scheme whose window nobody stated, so the rule filled the \
         choice in rather than asking for it"
    );
    let said = format!("{:?}", response.refusals);
    assert!(
        said.contains("window_index"),
        "the rule declined without naming window_index, so a caller is told a number is \
         missing and not which choice would produce it: {said}"
    );
}

#[test]
fn the_force_at_an_epoch_is_the_sample_the_recording_holds_there() {
    let trial = subject01_trial1();
    let epoch_milliseconds = 200.0;
    let response = run(
        &trial,
        &asking(
            "force_at_epoch",
            "force.at_epoch_from_onset",
            &[("epoch_ms", epoch_milliseconds)],
        ),
    )
    .expect("the request runs");
    let reported =
        number(&response, "force_at_epoch_newtons").expect("the rule reported the force");
    let onset = response.onset_index.expect("the onset rule placed one");
    let samples = (epoch_milliseconds / 1000.0 * SAMPLE_RATE_HZ).round() as usize;

    assert_eq!(
        reported,
        trial.force()[onset + samples],
        "the reported force is not the sample the recording holds {epoch_milliseconds} ms \
         after the onset this same response placed at {onset}"
    );

    // The rate over the same interval is the same chord read across its span. A reader
    // meeting both numbers is entitled to have them agree about which samples they came from.
    let rate = answered(
        &trial,
        RATE_CONSTRUCT,
        OVERLAPPING,
        &[("epoch_ms", epoch_milliseconds)],
        RATE_KEY,
    );
    let across_the_chord = (reported - trial.force()[onset]) / (epoch_milliseconds / 1000.0);
    assert!(
        (rate - across_the_chord).abs() < 1e-9,
        "the rate over this interval is {rate} N/s and the force this rule reports implies \
         {across_the_chord} N/s, so the two rules are reading different samples"
    );
}
