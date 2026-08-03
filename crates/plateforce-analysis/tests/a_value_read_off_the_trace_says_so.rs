//! A value that moves when the trace moves was read off the trace, and has to say so.
//!
//! `ParameterSource` separates a value the caller stated, one the rule fell back to, and one
//! the rule computed from this recording. The three are indistinguishable in the number, so
//! the only evidence available to a test is behavioural: under one identical request, a
//! stated value and a registry fallback are fixed, and a measured one follows the recording.
//!
//! Two runs of the same request over two recordings therefore partition the record. Every
//! name whose value differs between them was measured, whatever the record calls it.

use std::collections::{BTreeMap, BTreeSet};

use plateforce_analysis::{run, AnalysisRequest, MethodChoice, WeighingChoice, BINDINGS};
use plateforce_core::provenance::ParameterSource;
use plateforce_core::{Trial, STANDARD_GRAVITY_METERS_PER_SECOND_SQUARED};

/// Quiet stance, an unweighting dip, a push, flight, then landing, at a stated system weight,
/// sample rate, and quiet-stance length. All three move the landmarks a rule measures while
/// leaving every value a caller states exactly where it was.
fn trial(system_weight_newtons: f64, sample_rate_hz: f64, stance_seconds: f64) -> Trial {
    let stance_samples = (stance_seconds * sample_rate_hz) as usize;
    let push_samples = (0.3 * sample_rate_hz) as usize;
    let flight_samples = (0.5 * sample_rate_hz) as usize;
    let mut force = Vec::new();
    force.extend(
        (0..stance_samples).map(|index| system_weight_newtons + ((index % 17) as f64 - 8.0) * 0.4),
    );
    force.extend(
        (0..push_samples)
            .map(|index| system_weight_newtons * (1.0 - 0.5 * index as f64 / push_samples as f64)),
    );
    force.extend(
        (0..push_samples)
            .map(|index| system_weight_newtons * (0.5 + 2.0 * index as f64 / push_samples as f64)),
    );
    // The unloaded plate is noisy in proportion to what it was carrying, which is what a rule
    // that re-estimates its threshold from the flight phase reads.
    force.extend(
        (0..flight_samples)
            .map(|index| ((index % 11) as f64 - 5.0) * system_weight_newtons * 0.0004),
    );
    force.extend(std::iter::repeat_n(
        system_weight_newtons * 2.4,
        push_samples,
    ));
    Trial::new(force, sample_rate_hz).expect("the fixture is a well formed trial")
}

/// A sample index the caller states, which means a different number of seconds on recordings
/// at different rates. Stating it exercises the branch where a rule turns a caller's index
/// into a time, which is the caller's choice and the recording's arithmetic together.
const STATED_WEIGHING_START_INDEX: usize = 300;

/// Nothing stated beyond the rule under test, so every value in the record came from the rule
/// rather than from the caller.
fn request_for(slot: &str, method_id: &str) -> AnalysisRequest {
    let mut request = AnalysisRequest {
        // The weighing rule searches for its window rather than being handed one, so the
        // epoch's own bounds are measured and the rules downstream of it read measured times
        // rather than the same stated second on every recording.
        weighing: WeighingChoice {
            method_id: "bwepoch.adaptive_lowest_variance".into(),
            start_index: None,
            parameters: BTreeMap::new(),
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
    };
    match slot {
        "weighing" => {
            request.weighing.method_id = method_id.to_string();
            request.weighing.parameters = BTreeMap::new();
        }
        "onset" => {
            request.onset = MethodChoice {
                method_id: method_id.to_string(),
                ..Default::default()
            }
        }
        _ => {
            request.takeoff = MethodChoice {
                method_id: method_id.to_string(),
                ..Default::default()
            }
        }
    }
    request
}

/// Every `(method id, name)` the record carries for one recording, with the text shown for it
/// and the source the record claims.
fn record_over(trial: &Trial) -> BTreeMap<(String, String), (String, ParameterSource)> {
    let mut recorded = BTreeMap::new();
    for binding in BINDINGS {
        // Both branches of every rule that behaves differently when the caller anchors it,
        // because a rule can be honest about a value it chose and wrong about one it derived
        // from the caller's.
        for start_index in [None, Some(STATED_WEIGHING_START_INDEX)] {
            let mut request = request_for(binding.slot, binding.id);
            request.weighing.start_index = start_index;
            let Ok(response) = run(trial, &request) else {
                continue;
            };
            for bound in &response.bound_methods {
                for (name, shown) in &bound.bound_parameters {
                    let Some(source) = bound.parameter_sources.get(name) else {
                        continue;
                    };
                    recorded.insert(
                        (bound.method_id.clone(), name.clone()),
                        (shown.clone(), *source),
                    );
                }
            }
        }
    }
    recorded
}

#[test]
fn a_value_that_follows_the_recording_is_recorded_as_measured() {
    let lighter = record_over(&trial(600.0, 1000.0, 1.4));
    let heavier = record_over(&trial(824.0, 1200.0, 2.1));

    let mut followed_the_recording = BTreeSet::new();
    let mut misattributed = Vec::new();

    for (key, (shown, source)) in &lighter {
        let Some((other_shown, _)) = heavier.get(key) else {
            continue;
        };
        if shown == other_shown {
            continue;
        }
        followed_the_recording.insert(key.clone());
        if *source != ParameterSource::Measured {
            let (method_id, name) = key;
            misattributed.push(format!(
                "{method_id} records {name} as {source:?}, and it moved from {shown} to \
                 {other_shown} when only the recording changed"
            ));
        }
    }

    assert!(
        misattributed.is_empty(),
        "a value the rule read off the trace is recorded as something the caller or the \
         registry supplied:\n  {}",
        misattributed.join("\n  ")
    );
    assert!(
        followed_the_recording.len() >= 4,
        "only {} values moved with the recording, so this comparison has stopped reaching the \
         values it exists to check: {:?}",
        followed_the_recording.len(),
        followed_the_recording
    );
}
