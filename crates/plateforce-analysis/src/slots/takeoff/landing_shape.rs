//! `takeoff.threshold.landing_shape`: the first low-force run the recording closes with a
//! landing.

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
    RuleRefusal::Stated(format!(
        "takeoff.threshold.landing_shape read {} run{} below threshold_n = {} N and none of them \
         ends in a rise reaching landing_rise_rate_floor_bodyweights_per_second = {} and \
         landing_peak_floor_bodyweights = {}, with {} of them running to the end of the recording",
        runs.len(),
        if runs.len() == 1 { "" } else { "s" },
        crate::resolution::format_number(threshold_newtons),
        crate::resolution::format_number(spec.landing_rise_rate_floor_bodyweights_per_second),
        crate::resolution::format_number(spec.landing_peak_floor_bodyweights),
        open_ended,
    ))
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
