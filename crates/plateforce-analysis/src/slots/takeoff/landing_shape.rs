//! `takeoff.threshold.landing_shape`: the first low-force run the recording closes with a
//! landing.

use std::collections::BTreeMap;

use plateforce_core::takeoff::landing_shape::{
    classify_runs, takeoff_by_landing_shape, LandingShapeSpec,
};
use plateforce_core::{Trial, WeighingEpoch};

use crate::resolution::{Resolution, RuleRefusal};

/// Every setting the rule reads, taken from the request where the caller stated one and
/// from the fitted spec where they did not. The fallbacks are read off `LandingShapeSpec`
/// rather than restated, so the registry row's defaults have one home in the core.
fn spec(resolved: &mut Resolution) -> LandingShapeSpec {
    let fitted = LandingShapeSpec::default();
    LandingShapeSpec {
        landing_rise_rate_floor_bodyweights_per_second: resolved.number(
            "landing_rise_rate_floor_bodyweights_per_second",
            fitted.landing_rise_rate_floor_bodyweights_per_second,
        ),
        landing_peak_floor_bodyweights: resolved.number(
            "landing_peak_floor_bodyweights",
            fitted.landing_peak_floor_bodyweights,
        ),
        maximum_rise_seconds: resolved.number("maximum_rise_seconds", fitted.maximum_rise_seconds),
        slope_span_seconds: resolved.number("slope_span_seconds", fitted.slope_span_seconds),
        minimum_run_seconds: resolved.number("minimum_run_seconds", fitted.minimum_run_seconds),
        bridged_gap_seconds: resolved.number("bridged_gap_seconds", fitted.bridged_gap_seconds),
        bridged_gap_ceiling_bodyweights: resolved.number(
            "bridged_gap_ceiling_bodyweights",
            fitted.bridged_gap_ceiling_bodyweights,
        ),
    }
}

/// What the rule read, for a caller that has to say why it found nothing.
fn nothing_found(
    trial: &Trial,
    epoch: &WeighingEpoch,
    threshold_newtons: f64,
    spec: &LandingShapeSpec,
) -> RuleRefusal {
    let runs = classify_runs(
        trial.force(),
        epoch.system_weight_newtons,
        threshold_newtons,
        trial.sample_rate_hz(),
        spec,
    );
    let open_ended = runs.iter().filter(|run| run.ends_the_recording).count();
    // Every figure the sentence used to interpolate is a number a caller branches on, so
    // each one is a field. The count of runs reaching the end of the recording is the one
    // that says whether the trace was cut mid-flight rather than the floors being wrong.
    RuleRefusal::Refused(Box::new(plateforce_core::Refusal::nothing_qualified(
        "takeoff.threshold.landing_shape",
        runs.len(),
        BTreeMap::from([
            ("threshold_n".to_string(), threshold_newtons),
            (
                "landing_rise_rate_floor_bodyweights_per_second".to_string(),
                spec.landing_rise_rate_floor_bodyweights_per_second,
            ),
            (
                "landing_peak_floor_bodyweights".to_string(),
                spec.landing_peak_floor_bodyweights,
            ),
            (
                "runs_reaching_the_end_of_the_recording".to_string(),
                open_ended as f64,
            ),
        ]),
    )))
}

pub(crate) fn crossing(
    trial: &Trial,
    epoch: &WeighingEpoch,
    threshold_newtons: f64,
    resolved: &mut Resolution,
    warnings: &mut Vec<String>,
) -> Result<usize, RuleRefusal> {
    let spec = spec(resolved);
    let (placed, landing_count) = takeoff_by_landing_shape(
        trial.force(),
        epoch.system_weight_newtons,
        threshold_newtons,
        trial.sample_rate_hz(),
        &spec,
    );
    match placed {
        // The rule takes the first of several rather than choosing between them, so a
        // recording holding more than one jump says so instead of reporting one silently.
        Some(index) => {
            if landing_count > 1 {
                warnings.push(format!(
                    "{landing_count} low-force runs in this recording end in a landing, and takeoff is placed on the first of them"
                ));
            }
            Ok(index)
        }
        None => Err(nothing_found(trial, epoch, threshold_newtons, &spec)),
    }
}
