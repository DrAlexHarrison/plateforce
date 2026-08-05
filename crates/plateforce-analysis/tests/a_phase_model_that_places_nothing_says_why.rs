//! A phase model on a recording it cannot read, and what the reader is told.
//!
//! A model composes several searches and publishes one quantity per boundary. Where a search
//! comes back empty the model has no interval to assert, so it declines, and the reason names
//! the search rather than the model: two recordings stop two different searches and call for
//! two different repairs.
//!
//! Before this the declining path published one null per boundary and no refusal at all. Five
//! empty cells were the whole of a reader's answer on `synthetic_untrimmed_step_off`, on the
//! surface built to refuse rather than guess.

use std::collections::BTreeMap;

use plateforce_analysis::{run, AnalysisRequest, AnalysisResponse, MethodChoice, WeighingChoice};
use plateforce_core::Trial;

const SINGLE: &str = "phase.model.unweighting_single.mcmahon2018";
const SPLIT: &str = "phase.model.unloading_yielding_split.harry2020";
const PHASE_MODEL: &str = "phase_model";

/// A countermovement jump with a landing: quiet stance, an unweighting dip, a braking rise
/// through system weight, a propulsive peak, flight, and a landing.
///
/// The control's trace. Every search both models compose finds something here.
fn a_countermovement_jump() -> Trial {
    let mut force = quiet_stance();
    force.extend((0..360).map(|index| 600.0 - 300.0 * (index as f64 / 360.0)));
    force.extend((0..360).map(|index| 300.0 + 1200.0 * (index as f64 / 360.0)));
    force.extend(std::iter::repeat_n(0.0, 600));
    force.extend(std::iter::repeat_n(2400.0, 240));
    force.extend(std::iter::repeat_n(600.0, 600));
    Trial::new(force, 1200.0).unwrap()
}

/// The same jump with the countermovement taken out: the athlete pushes from standing without
/// dipping first.
///
/// Force never falls below system weight, so both models' first search reads the whole
/// interval and finds nothing. The two searches are not the same search, which is what
/// separates a refusal that names one from a refusal that names the model.
fn a_push_with_no_countermovement() -> Trial {
    let mut force = quiet_stance();
    force.extend((0..360).map(|index| 600.0 + 1200.0 * (index as f64 / 360.0)));
    force.extend(std::iter::repeat_n(0.0, 600));
    force.extend(std::iter::repeat_n(2400.0, 240));
    force.extend(std::iter::repeat_n(600.0, 600));
    Trial::new(force, 1200.0).unwrap()
}

/// One second of standing at 600 N, dithered so the noise-relative onset rule has a band to
/// measure. The dither spans 6.4 N, which is well above the 15 N the split model's unloading
/// level sits below system weight, so the quiet stretch is not itself a departure.
fn quiet_stance() -> Vec<f64> {
    let mut force = vec![600.0; 1200];
    for (index, sample) in force.iter_mut().enumerate() {
        *sample += ((index % 17) as f64 - 8.0) * 0.4;
    }
    force
}

fn naming(model: &str) -> AnalysisRequest {
    let mut request = AnalysisRequest {
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
    };
    request.derived.insert(
        PHASE_MODEL.to_string(),
        MethodChoice {
            method_id: model.to_string(),
            ..Default::default()
        },
    );
    request
}

fn refusal_from(response: &AnalysisResponse, model: &str) -> String {
    let declined: Vec<String> = response
        .refusals
        .iter()
        .filter(|declined| declined.method_id == model)
        .map(|declined| declined.refusal.to_string())
        .collect();
    assert_eq!(
        declined.len(),
        1,
        "{model} placed nothing and left {} refusals, so a reader has {}",
        declined.len(),
        if declined.is_empty() {
            "no reason at all"
        } else {
            "two accounts of one cause"
        }
    );
    declined[0].clone()
}

fn filled(response: &AnalysisResponse, model: &str) -> Vec<String> {
    response
        .metrics
        .iter()
        .filter(|metric| metric.computed_by.as_deref() == Some(model))
        .filter(|metric| metric.value.is_some())
        .map(|metric| metric.key.clone())
        .collect()
}

/// The claim, on both models: a search that found nothing is a refusal naming that search.
#[test]
fn a_model_whose_search_found_nothing_names_the_search() {
    let trial = a_push_with_no_countermovement();

    let single = run(&trial, &naming(SINGLE)).expect("the request is bound");
    let single_reason = refusal_from(&single, SINGLE);
    assert!(
        single_reason.contains("departure below system weight"),
        "the single model declined without naming its own search: {single_reason}"
    );
    assert!(
        filled(&single, SINGLE).is_empty(),
        "a model that placed no boundary published a number for one"
    );

    let split = run(&trial, &naming(SPLIT)).expect("the request is bound");
    let split_reason = refusal_from(&split, SPLIT);
    assert!(
        split_reason.contains("departure below the unloading level"),
        "the split model declined without naming its own search: {split_reason}"
    );
    assert!(filled(&split, SPLIT).is_empty());

    // The two models stopped on two searches, so the sentence is read off what the model
    // looked for and is not one string every declining model shares. Written as an inequality
    // rather than as two `contains` alone: both assertions above pass against a build that
    // names every search after the one it happens to reach first.
    assert_ne!(
        single_reason, split_reason,
        "both models decline in one sentence, so it names neither search"
    );
}

/// The control, and it can come back empty for the same reason the guard can: the same two
/// models on a jump they can read place their boundaries and refuse nothing.
///
/// Without it a build that refused every phase model on every recording satisfies every
/// assertion above.
#[test]
fn a_model_that_read_the_recording_refuses_nothing() {
    let trial = a_countermovement_jump();

    for model in [SINGLE, SPLIT] {
        let response = run(&trial, &naming(model)).expect("the request is bound");
        let declined: Vec<String> = response
            .refusals
            .iter()
            .filter(|declined| declined.method_id == model)
            .map(|declined| declined.refusal.to_string())
            .collect();
        assert!(
            declined.is_empty(),
            "{model} read a jump it can place and declined anyway: {declined:?}"
        );
        assert!(
            !filled(&response, model).is_empty(),
            "{model} refused nothing and published no boundary either"
        );
    }
}
