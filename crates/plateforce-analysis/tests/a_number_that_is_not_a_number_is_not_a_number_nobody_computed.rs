//! Which kind of nothing a result reports, on the wire.
//!
//! `serde_json` writes a non-finite float as `null`, exactly as it writes a quantity no rule
//! produced. So "the software computed this and got not a number" and "the software declined
//! to compute this" reach a reader as the same three characters, and a field beside the value
//! is what tells them apart. The first is a gap in the recording reaching a number; the second
//! is a refusal that `refusals` accounts for by name.
//!
//! The recording is the one built for it. `subject01_trial1_interrupted` carries three samples
//! that are not numbers, at zero-based indices 300, 301 and 302, inside the one-second weighing
//! window this request binds. So the quantities computed over that window are not numbers and
//! the quantities that rest on an onset nobody found have no value at all, and one recording
//! puts both states in one document.
//!
//! The control is the same request against the intact recording, where neither state occurs.
//! It can come back empty for the same reason the real query would, because it runs the same
//! rules through the same code and reads the same fields.

use std::collections::{BTreeMap, BTreeSet};

use plateforce_analysis::{run, AnalysisRequest, AnalysisResponse, MethodChoice, WeighingChoice};
use plateforce_core::{read_trial_from_path, Trial};

const INTERRUPTED: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../plateforce-conformance/damaged/subject01_trial1_interrupted.force.txt"
);
const INTACT: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../plateforce-conformance/fixtures/subject01_trial1.force.txt"
);

const CORPUS_SAMPLE_RATE_HZ: f64 = 1200.0;

/// The samples of the interrupted recording that carry no number, counted by walking the
/// trace rather than by reading it out of the file, so the fixture and the expectation cannot
/// drift apart quietly.
const SAMPLES_CARRYING_NO_NUMBER: usize = 3;

/// The two quantities computed over the weighing window. The window is fixed at one second
/// from the start of the recording and the gap sits at samples 300 to 302, so these two are
/// the ones the gap reaches. Compared as a set rather than as a count: a third quantity
/// arriving in this state is a mechanism nobody has accounted for and is worth reddening for.
const REACHED_BY_THE_GAP: [&str; 2] = ["system_weight_newtons", "system_mass_kilograms"];

fn trial(path: &str) -> Trial {
    let (trial, _) = read_trial_from_path(path, '\t', 0, CORPUS_SAMPLE_RATE_HZ)
        .unwrap_or_else(|error| panic!("{path} did not read: {error}"));
    trial
}

/// The request the committed parity population asks of this recording, so this guard and the
/// cross-surface gate are asking one question.
fn request() -> AnalysisRequest {
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
            parameters: BTreeMap::from([("threshold_n".to_string(), 20.0)]),
            ..Default::default()
        },
        ..Default::default()
    }
}

fn analysed(path: &str) -> AnalysisResponse {
    run(&trial(path), &request()).expect("this request produces a document on both recordings")
}

fn carrying_no_number(response: &AnalysisResponse) -> BTreeSet<String> {
    response
        .metrics
        .iter()
        .filter(|metric| metric.carried_no_number)
        .map(|metric| metric.key.clone())
        .collect()
}

fn without_a_value(response: &AnalysisResponse) -> BTreeSet<String> {
    response
        .metrics
        .iter()
        .filter(|metric| metric.value.is_none() && !metric.carried_no_number)
        .map(|metric| metric.key.clone())
        .collect()
}

/// The property, and it is about the wire rather than about the structs.
///
/// Both states write `null` under `value`, which is what makes them indistinguishable without
/// the field beside it. So the assertion reads the serialised document, confirms every metric
/// in both states writes `null`, and confirms the two are told apart there.
#[test]
fn the_wire_tells_a_number_that_is_not_a_number_from_a_number_nobody_computed() {
    let response = analysed(INTERRUPTED);
    let document: serde_json::Value =
        serde_json::from_str(&serde_json::to_string(&response).expect("the response serialises"))
            .expect("what serde wrote, serde reads");

    let mut not_a_number = BTreeSet::new();
    let mut no_value = BTreeSet::new();
    for metric in document["metrics"]
        .as_array()
        .expect("metrics is an array on the wire")
    {
        let key = metric["key"].as_str().expect("every metric names itself");
        if !metric["value"].is_null() {
            continue;
        }
        // The reason the field is needed: the wire value is the same for both, so the state
        // is unreadable without it.
        assert_eq!(metric["value"], serde_json::Value::Null);
        match metric["carried_no_number"]
            .as_bool()
            .unwrap_or_else(|| panic!("{key} publishes no carried_no_number"))
        {
            true => not_a_number.insert(key.to_string()),
            false => no_value.insert(key.to_string()),
        };
    }

    assert_eq!(
        not_a_number,
        REACHED_BY_THE_GAP
            .iter()
            .map(|key| (*key).to_string())
            .collect::<BTreeSet<String>>(),
        "the quantities computed over the window holding the gap are the ones that are not \
         numbers"
    );
    assert!(
        !no_value.is_empty(),
        "this recording also suppresses quantities that rest on an onset nobody found, and a \
         document with only one of the two states cannot show that they are told apart"
    );
    assert!(
        not_a_number.is_disjoint(&no_value),
        "a metric cannot be in both states"
    );
}

/// The control. Same rules, same code, intact recording, and neither state occurs, so the
/// buckets above can only fill on a recording with a gap in it.
#[test]
fn an_intact_recording_puts_a_metric_in_neither_state() {
    let response = analysed(INTACT);
    assert_eq!(carrying_no_number(&response), BTreeSet::new());
    assert_eq!(without_a_value(&response), BTreeSet::new());
    assert_eq!(
        response
            .metrics
            .iter()
            .filter(|m| m.value.is_some())
            .count(),
        response.metrics.len(),
        "every quantity this request asks for has a value on the intact recording"
    );
}

/// The contract the rest of the crate rests on: a metric's value is a number or it is
/// absent, never a value that is not finite. Held here so no call site downstream decides it
/// again.
#[test]
fn no_metric_carries_a_value_that_is_not_a_number() {
    for path in [INTERRUPTED, INTACT] {
        let response = analysed(path);
        for metric in &response.metrics {
            assert!(
                metric.value.is_none_or(|value| value.is_finite()),
                "{path}: {} carries {:?} in a field that promises a number",
                metric.key,
                metric.value
            );
        }
    }
}

/// The count of what the recording lost, on the response, so every surface publishes it from
/// one place rather than four.
///
/// Counted over the recording as it arrived. The control is the intact recording reading zero,
/// which is what says the counter counts rather than reporting the length of something.
#[test]
fn the_response_counts_the_samples_the_recording_lost() {
    assert_eq!(
        analysed(INTERRUPTED).samples_carrying_no_number,
        SAMPLES_CARRYING_NO_NUMBER
    );
    assert_eq!(analysed(INTACT).samples_carrying_no_number, 0);

    // Taken from the trace rather than from the constant, so the fixture growing or losing a
    // hole moves the expectation with it instead of leaving this guard asserting history.
    let walked = trial(INTERRUPTED)
        .force()
        .iter()
        .filter(|value| !value.is_finite())
        .count();
    assert_eq!(walked, SAMPLES_CARRYING_NO_NUMBER);
}

/// The levels an interface draws are numbers or they are absent. A level typed as a plain
/// `f64` serialises as `null` while its type promises a number on every result.
///
/// The weighing standard deviation is not a reported quantity, so it has no metric of its own
/// to carry the distinction. It shares its window with system weight, and the assertion holds
/// the two together: when the window's weight is not a number its dispersion is not one
/// either, and the metric of the same key is what says which kind of `null` both are.
#[test]
fn the_weighing_statistics_and_their_metrics_agree_about_having_no_number() {
    let response = analysed(INTERRUPTED);
    assert_eq!(response.levels.system_weight_newtons, None);
    assert_eq!(response.levels.weighing_standard_deviation_newtons, None);
    assert!(
        response
            .metric("system_weight_newtons")
            .expect("the quantity is reported whether or not it has a value")
            .carried_no_number,
        "the level is absent because the arithmetic produced no number, and the metric is \
         where a reader is told so"
    );

    let intact = analysed(INTACT);
    for level in [
        intact.levels.system_weight_newtons,
        intact.levels.weighing_standard_deviation_newtons,
        intact.levels.onset_band_lower_newtons,
        intact.levels.onset_band_upper_newtons,
        intact.levels.takeoff_threshold_newtons,
    ] {
        assert!(level.is_some_and(f64::is_finite), "{level:?}");
    }
    assert!(
        !intact
            .metric("system_weight_newtons")
            .expect("the quantity is reported")
            .carried_no_number
    );
}
