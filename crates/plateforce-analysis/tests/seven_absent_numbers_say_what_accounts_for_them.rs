//! What a reader is told when the two landmarks come back in the wrong order.
//!
//! On a recording where the athlete steps off the plate after landing, one published onset rule
//! places the start of the jump on the step-off, two seconds after the takeoff another rule
//! found in the jump itself. The rule searches back from the recording's force extremum and
//! keeps the last departure from quiet before it, which its entry says in those words, and on
//! an untrimmed trace the emptiest the plate ever reads is the athlete standing beside it. The
//! analysis is right to measure nothing across an interval that runs backwards, and it
//! suppresses seven quantities. Until this signal it said so in one sentence in `warnings`,
//! carrying no status, no value and no list of the columns it accounted for, so a reader
//! holding seven blank cells could not tell which of them that sentence was about.
//!
//! The property that matters is the second test: the columns the signal names are exactly the
//! columns that came back without a value. A signal naming fewer would leave a blank cell with
//! nothing pointing at it, and one naming more would claim a number was absent when it is
//! sitting in the table.
//!
//! The recording is the one built for this, not the sibling whose step-off comes first. Swept
//! at their shipped defaults, 8 of the 50 combinations of 2 weighing rules, 5 onset rules and 5
//! takeoff rules invert the landmarks on it, and 0 of the 12,300 the 246-trial corpus offers do,
//! because every trial in that corpus was trimmed to a single jump before it was archived.

use std::collections::{BTreeMap, BTreeSet};

use plateforce_analysis::quality::{QualitySignal, QualityStatus};
use plateforce_analysis::{run, AnalysisRequest, AnalysisResponse, MethodChoice, WeighingChoice};
use plateforce_core::{read_trial_from_path, Trial};

const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../plateforce-conformance/fixtures/synthetic_untrimmed_step_off_after_jump.force.txt"
);

const CORPUS_SAMPLE_RATE_HZ: f64 = 1200.0;

/// The instants these rules place on this recording. Onset lands on the step-off, which is
/// after the landing, and takeoff on the jump's own flight.
const ONSET_SECONDS: f64 = 3.5433333333333334;
const TAKEOFF_SECONDS: f64 = 1.8566666666666667;

fn trial() -> Trial {
    let (trial, _) = read_trial_from_path(FIXTURE, '\t', 0, CORPUS_SAMPLE_RATE_HZ)
        .unwrap_or_else(|error| panic!("{FIXTURE} did not read: {error}"));
    trial
}

/// The rules that place onset on the step-off, and the rules that do not. Only the trio
/// changes between the two runs, on one recording.
fn analyse(weighing: &str, window: &str, onset: &str, takeoff: &str) -> AnalysisResponse {
    let request = AnalysisRequest {
        weighing: WeighingChoice {
            method_id: weighing.into(),
            parameters: BTreeMap::from([(window.to_string(), 1.0)]),
            ..Default::default()
        },
        onset: MethodChoice {
            method_id: onset.into(),
            parameters: BTreeMap::from([
                ("k".to_string(), 5.0),
                ("window_seconds".to_string(), 1.0),
            ]),
            ..Default::default()
        },
        takeoff: MethodChoice {
            method_id: takeoff.into(),
            parameters: BTreeMap::from([("threshold_n".to_string(), 20.0)]),
            ..Default::default()
        },
        ..Default::default()
    };
    run(&trial(), &request).unwrap_or_else(|refusal| panic!("{onset} did not run: {refusal}"))
}

/// The pair whose onset rule reads the step-off as the start of the jump.
fn landmarks_out_of_order() -> AnalysisResponse {
    analyse(
        "bwepoch.adaptive_lowest_variance",
        "window_seconds",
        "onset.threshold.adaptive_trailing_window",
        "takeoff.threshold.flight_noise_k_sd",
    )
}

/// The same recording read by rules that place both landmarks in the order they happened.
fn landmarks_in_order() -> AnalysisResponse {
    analyse(
        "bwepoch.fixed_window",
        "duration",
        "onset.threshold.noise_relative",
        "takeoff.threshold.absolute_force",
    )
}

fn raised(response: &AnalysisResponse) -> Vec<&QualitySignal> {
    response
        .signals
        .iter()
        .filter(|signal| signal.status == QualityStatus::OnsetNotBeforeTakeoff)
        .collect()
}

/// Every quantity the response reported with no value.
fn absent_keys(response: &AnalysisResponse) -> BTreeSet<&str> {
    response
        .metrics
        .iter()
        .filter(|metric| metric.value.is_none())
        .map(|metric| metric.key.as_str())
        .collect()
}

#[test]
fn landmarks_that_came_back_out_of_order_are_said_rather_than_left_to_the_blank_cells() {
    let response = landmarks_out_of_order();
    let onset = response.onset_index.expect("the onset rule answered");
    let takeoff = response.takeoff_index.expect("the takeoff rule answered");
    println!("onset sample {onset}, takeoff sample {takeoff}");
    assert!(
        onset >= takeoff,
        "the rules no longer place the landmarks out of order on this recording, so this \
         guard is asserting against a case it can no longer reach"
    );

    let signals = raised(&response);
    let signal = signals.first().expect("the condition is reported");
    println!(
        "{}: {:?} against {}",
        signal.label, signal.value, signal.threshold
    );
    assert_eq!(signals.len(), 1, "said more than once: {signals:#?}");

    // Both instants, so every surface that prints a value against a threshold prints two
    // numbers that are true of this recording.
    assert!((signal.value.expect("an instant") - ONSET_SECONDS).abs() < 1e-9);
    assert!((signal.threshold - TAKEOFF_SECONDS).abs() < 1e-9);
    assert!(
        signal.value.expect("an instant") >= signal.threshold,
        "the signal fires when onset is at or after takeoff, so its value is never the earlier \
         of the two"
    );
    assert_eq!(signal.unit, "seconds");
    assert_eq!(signal.remedy_construct, "takeoff");

    // Not a refusal. Both rules answered, and the trial still carries what was measured
    // outside the interval.
    assert!(response.refusals.is_empty(), "{:#?}", response.refusals);
    assert!(response
        .metric("system_weight_newtons")
        .is_some_and(|metric| metric.value.is_some()));
}

/// The assertion the whole signal exists for, and the one that cannot pass by accident: the
/// columns it names are read off the signal and the columns without values are read off the
/// response, and the two sets are compared.
#[test]
fn the_columns_the_signal_names_are_exactly_the_columns_that_came_back_empty() {
    let response = landmarks_out_of_order();
    let absent = absent_keys(&response);
    let signals = raised(&response);
    let named: BTreeSet<&str> = signals
        .first()
        .expect("the condition is reported")
        .qualifies
        .iter()
        .copied()
        .collect();

    println!("absent: {absent:?}");
    println!("named:  {named:?}");
    assert_eq!(
        named, absent,
        "a column came back empty that the signal does not account for, or the signal names a \
         column that carries a number"
    );
    assert_eq!(absent.len(), 7, "the recording no longer suppresses seven");
}

/// The other half of the same recording. A guard that fired on every trial would pass the two
/// tests above, and this is the run where the landmarks are in the order they happened.
#[test]
fn the_same_recording_read_by_rules_that_order_the_landmarks_says_nothing() {
    let response = landmarks_in_order();
    let onset = response.onset_index.expect("the onset rule answered");
    let takeoff = response.takeoff_index.expect("the takeoff rule answered");
    println!("onset sample {onset}, takeoff sample {takeoff}");
    assert!(
        onset < takeoff,
        "the control no longer orders the landmarks"
    );

    assert!(raised(&response).is_empty(), "{:#?}", response.signals);
    // And the quantities are all there, so the absence above is the condition rather than
    // something this recording does whatever it is read with.
    assert!(
        absent_keys(&response).is_empty(),
        "{:?}",
        absent_keys(&response)
    );
}

/// The word travels, and it travels from the one home rather than from a spelling written
/// here. Every surface but the terminal reads the status off the record.
#[test]
fn the_status_reaches_a_reader_under_the_name_the_record_carries() {
    assert_eq!(
        QualityStatus::OnsetNotBeforeTakeoff.wire_name(),
        "onset_not_before_takeoff"
    );
    let response = landmarks_out_of_order();
    let text = serde_json::to_string(&response.signals).expect("the signals serialise");
    assert!(text.contains("onset_not_before_takeoff"), "{text}");
}
