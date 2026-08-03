//! What an onset rule that returned its own boundary has to say about it.
//!
//! Two open tools place takeoff 843 ms late on 155 of 244 trials and say nothing, which is
//! the observation this project was founded on. On subject 01's first trial two of the five
//! published onset rules reproduce that shape here: correct arithmetic, a faithful reading
//! of the published rule, and an answer that is the floor of the rule's own search rather
//! than anything found in the recording.
//!
//! So the property is not that a signal exists. It is that the two rules which returned
//! their floor say so and the three which found a departure stay quiet, on one recording,
//! under one weighing window, with only the onset rule changing between runs.

use std::collections::BTreeMap;

use plateforce_analysis::quality::QualityStatus;
use plateforce_analysis::{run, AnalysisRequest, AnalysisResponse, MethodChoice, WeighingChoice};
use plateforce_core::{read_trial_from_path, Trial};

const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../plateforce-conformance/fixtures/subject01_trial1.force.txt"
);

/// The founding corpus samples at 1200 Hz. Read at 1000 every landmark index below moves.
const CORPUS_SAMPLE_RATE_HZ: f64 = 1200.0;

/// The weighing window this whole file runs under, which is what places the floor at sample
/// 1200, and the fixed backward offset every one of these rules composes, which is 36
/// samples at this rate. 1200 - 36 = 1164, and that is the arithmetic the guard is about.
const WEIGHING_WINDOW_SECONDS: f64 = 1.0;
const EXPECTED_FLOOR_INDEX: usize = 1200;
const EXPECTED_BACKWARD_OFFSET_SAMPLES: usize = 36;

/// The five onset rules this build ships, with the parameters each reads stated rather than
/// left to a fallback, so a run says what it ran.
fn onset_rules() -> Vec<(&'static str, BTreeMap<String, f64>)> {
    vec![
        (
            "onset.threshold.noise_relative",
            BTreeMap::from([("k".to_string(), 5.0)]),
        ),
        (
            "onset.threshold.relative_to_system_weight",
            BTreeMap::from([("pct".to_string(), 2.5)]),
        ),
        (
            "onset.threshold.absolute_force",
            BTreeMap::from([("threshold_n".to_string(), 20.0)]),
        ),
        (
            "onset.threshold.last_within_band",
            BTreeMap::from([("k".to_string(), 5.0)]),
        ),
        (
            "onset.threshold.adaptive_trailing_window",
            BTreeMap::from([("k".to_string(), 5.0), ("window_seconds".to_string(), 1.0)]),
        ),
    ]
}

/// The two rules whose threshold the quiet stance on this recording already sits outside, so
/// the first sample they are permitted to examine already satisfies them.
const RULES_THAT_RETURN_THEIR_FLOOR: &[&str] = &[
    "onset.threshold.relative_to_system_weight",
    "onset.threshold.absolute_force",
];

fn trial() -> Trial {
    let (trial, _) = read_trial_from_path(FIXTURE, '\t', 0, CORPUS_SAMPLE_RATE_HZ)
        .unwrap_or_else(|error| panic!("{FIXTURE} did not read: {error}"));
    trial
}

fn analyse(trial: &Trial, onset_id: &str, parameters: BTreeMap<String, f64>) -> AnalysisResponse {
    let request = AnalysisRequest {
        weighing: WeighingChoice {
            method_id: "bwepoch.fixed_window".into(),
            parameters: BTreeMap::from([("duration".to_string(), WEIGHING_WINDOW_SECONDS)]),
            ..Default::default()
        },
        onset: MethodChoice {
            method_id: onset_id.to_string(),
            parameters,
            ..Default::default()
        },
        takeoff: MethodChoice {
            method_id: "takeoff.threshold.absolute_force".into(),
            parameters: BTreeMap::from([("threshold_n".to_string(), 20.0)]),
            ..Default::default()
        },
        ..Default::default()
    };
    run(trial, &request).unwrap_or_else(|refusal| panic!("{onset_id} did not run: {refusal}"))
}

fn floor_signals(response: &AnalysisResponse) -> Vec<&plateforce_analysis::quality::QualitySignal> {
    response
        .signals
        .iter()
        .filter(|signal| signal.status == QualityStatus::AtSearchFloor)
        .collect()
}

/// Both sides of the same run. A guard that only asserted the two rules speak would pass on
/// a signal attached to every result, and one that only asserted the three stay quiet would
/// pass on a signal that never fires at all.
#[test]
fn the_rules_that_returned_their_floor_say_so_and_the_rules_that_found_a_departure_do_not() {
    let trial = trial();
    let mut spoke: Vec<&str> = Vec::new();
    let mut quiet: Vec<&str> = Vec::new();

    for (onset_id, parameters) in onset_rules() {
        let response = analyse(&trial, onset_id, parameters);
        let raised = floor_signals(&response);
        println!(
            "{onset_id}: onset index {:?}, floor-landing signals {}",
            response.onset_index,
            raised.len()
        );
        assert!(
            raised.len() <= 1,
            "{onset_id} raised the same signal more than once: {raised:#?}"
        );
        if raised.is_empty() {
            quiet.push(onset_id);
        } else {
            spoke.push(onset_id);
        }
    }

    assert_eq!(
        spoke, RULES_THAT_RETURN_THEIR_FLOOR,
        "the rules that speak are not the rules that returned their floor"
    );
    assert_eq!(
        quiet.len(),
        onset_rules().len() - RULES_THAT_RETURN_THEIR_FLOOR.len(),
        "{quiet:?} stayed quiet"
    );
}

/// The measurement the signal rests on, asserted in samples rather than in seconds. Seconds
/// agreeing to four decimals could be two numbers that happen to round together. An integer
/// index agreeing exactly cannot.
#[test]
fn the_onset_those_rules_report_is_the_floor_with_the_composed_offset_put_back() {
    let trial = trial();
    for (onset_id, parameters) in onset_rules() {
        let response = analyse(&trial, onset_id, parameters);
        let onset_index = response.onset_index.expect("every rule placed an onset");
        let crossing = onset_index + EXPECTED_BACKWARD_OFFSET_SAMPLES;
        let returned_its_floor = RULES_THAT_RETURN_THEIR_FLOOR.contains(&onset_id);
        println!(
            "{onset_id}: onset {onset_index}, crossing {crossing}, floor {}",
            response.weighing_end_index
        );
        assert_eq!(
            response.weighing_end_index, EXPECTED_FLOOR_INDEX,
            "the weighing window moved, so the floor these rules share is not the one this \
             guard was measured against"
        );
        // Two of the five resolve their own backtrack, so the arithmetic is only claimed of
        // the rules that compose the fixed one. Both of those are floor-returning here.
        if returned_its_floor {
            assert_eq!(
                crossing, EXPECTED_FLOOR_INDEX,
                "{onset_id} was expected to have crossed on the first sample it could examine"
            );
        } else {
            assert_ne!(
                crossing, EXPECTED_FLOOR_INDEX,
                "{onset_id} crossed on the first sample it could examine after all"
            );
        }
    }
}

/// A signal is read, so what it hands a reader is part of it. Every surface prints the value
/// against the threshold in one sentence, and a value that did not exceed its threshold
/// would print a sentence that is not true.
#[test]
fn the_signal_hands_the_reader_a_value_a_surface_can_state_and_a_construct_it_can_open() {
    let trial = trial();
    let response = analyse(
        &trial,
        "onset.threshold.absolute_force",
        BTreeMap::from([("threshold_n".to_string(), 20.0)]),
    );
    let raised = floor_signals(&response);
    let signal = raised.first().expect("the rule returned its floor");
    println!("{}: {:?} {}", signal.label, signal.value, signal.unit);

    let value = signal
        .value
        .expect("a floor is an instant, so it has a value");
    assert!(
        value > signal.threshold,
        "value {value} against threshold {}",
        signal.threshold
    );
    assert_eq!(signal.unit, "seconds");
    assert_eq!(signal.remedy_construct, "movement_onset");
    assert!(
        signal.qualifies.contains(&"onset_time_seconds"),
        "{:?}",
        signal.qualifies
    );

    // Not a refusal. The arithmetic is right and the rule did what it publishes, so the
    // numbers stand and the reader decides.
    assert!(response.refusals.is_empty(), "{:#?}", response.refusals);
    assert!(
        response
            .metric("time_to_takeoff_seconds")
            .is_some_and(|metric| metric.value.is_some()),
        "the result still carries the number the signal is about"
    );
}
