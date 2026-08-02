//! Where the athlete leaves the plate, and the threshold every later comparison runs
//! against.

pub mod absolute_force;
pub mod descending_crossing;
pub mod flight_noise_k_sd;
pub mod longest_run;

use plateforce_core::{Trial, WeighingEpoch};

use crate::binding::unbound_method_message;
use crate::request::MethodChoice;
use crate::resolution::{BoundValues, Resolution, RuleRefusal};

pub(crate) struct TakeoffOutcome {
    pub index: Option<usize>,
    pub threshold_newtons: f64,
    pub bound: BoundValues,
    pub refusal: Option<RuleRefusal>,
}

/// The re-estimating rule seeds itself from the bounding rule's threshold, which the
/// registry files separately from the residual threshold the other three compare against.
fn seed_threshold(method_id: &str, resolved: &mut Resolution) -> f64 {
    match method_id {
        "takeoff.threshold.flight_noise_k_sd" => resolved.number(
            flight_noise_k_sd::SEED_PARAMETER,
            flight_noise_k_sd::SEED_DEFAULT_NEWTONS,
        ),
        _ => resolved.number(
            absolute_force::SEED_PARAMETER,
            absolute_force::SEED_DEFAULT_NEWTONS,
        ),
    }
}

/// Which core function the takeoff id names, and what it was given. The threshold is
/// carried in and out because the re-estimating rule replaces its own seed with one
/// measured from the flight phase, and touchdown is found against that one.
fn crossing(
    trial: &Trial,
    epoch: &WeighingEpoch,
    choice: &MethodChoice,
    resolved: &mut Resolution,
    threshold_newtons: &mut f64,
    warnings: &mut Vec<String>,
) -> Result<usize, RuleRefusal> {
    match choice.method_id.as_str() {
        "takeoff.threshold.longest_run" => {
            longest_run::crossing(trial, *threshold_newtons, resolved, warnings)
        }
        "takeoff.threshold.descending_crossing" => {
            descending_crossing::crossing(trial, *threshold_newtons, resolved)
        }
        "takeoff.threshold.flight_noise_k_sd" => {
            flight_noise_k_sd::crossing(trial, epoch, threshold_newtons, resolved)
        }
        "takeoff.threshold.absolute_force" => {
            absolute_force::crossing(trial, epoch, *threshold_newtons, resolved)
        }
        other => Err(RuleRefusal::Stated(unbound_method_message(
            other, "takeoff",
        ))),
    }
}

pub(crate) fn resolve(
    trial: &Trial,
    epoch: &WeighingEpoch,
    choice: &MethodChoice,
    warnings: &mut Vec<String>,
) -> TakeoffOutcome {
    let mut resolved = Resolution::over(
        &choice.parameters,
        &choice.options,
        &choice.recommended,
        &choice.from_registry_default,
    );
    let mut threshold_newtons = seed_threshold(&choice.method_id, &mut resolved);
    let index = crossing(
        trial,
        epoch,
        choice,
        &mut resolved,
        &mut threshold_newtons,
        warnings,
    );

    let bound = resolved.finish();
    match index {
        Ok(index) => TakeoffOutcome {
            index: Some(index),
            threshold_newtons,
            bound,
            refusal: None,
        },
        Err(rejected) => {
            warnings.push(rejected.to_string());
            TakeoffOutcome {
                index: None,
                threshold_newtons,
                bound,
                refusal: Some(rejected),
            }
        }
    }
}
