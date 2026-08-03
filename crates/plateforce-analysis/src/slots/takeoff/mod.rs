//! Where the athlete leaves the plate, and the threshold every later comparison runs
//! against.

pub mod absolute_force;
pub mod descending_crossing;
pub mod flight_noise_k_sd;
pub mod landing_shape;
pub mod longest_run;

use plateforce_core::{Trial, WeighingEpoch};

use crate::request::{AnalysisRequest, MethodChoice};
use crate::resolution::{BoundMethod, BoundValues, Resolution, RuleRefusal};

// Written once and read from here by everything that names one, for the same reason the
// onset family is: a list and a match spelling the same id are free to drift apart.
pub(crate) const TAKEOFF_OP_CROSSING_SELECTION: &str = "takeoff.op.crossing_selection";
pub(crate) const TAKEOFF_OP_SHORT_RUN_HANDLING: &str = "takeoff.op.short_run_handling";
pub(crate) const TAKEOFF_OP_RESIDUAL_COMPARISON: &str = "takeoff.op.residual_comparison";

/// The entries this build composes onto a takeoff threshold rule. Each is a registry entry
/// in its own right, filed under the takeoff construct rather than the onset one.
pub const TAKEOFF_OPERATOR_IDS: &[&str] = &[
    TAKEOFF_OP_CROSSING_SELECTION,
    TAKEOFF_OP_RESIDUAL_COMPARISON,
    TAKEOFF_OP_SHORT_RUN_HANDLING,
];

/// Which registry entry carries each name a takeoff rule reads.
///
/// The threshold rule carries its threshold and its persistence span, which its entry
/// publishes. The rest are operators with entries of their own, and until they were routed
/// here every one of them rode on a threshold row that does not list it: a reader looking up
/// the rule found no `comparison` and no `short_run_handling`, which between them decide
/// whether an unloaded plate reading negative counts as flight and whether a run too short
/// to be a flight can win the comparison and disqualify the trial.
fn operator_for(name: &str) -> Option<&'static str> {
    match name {
        "comparison" => Some(TAKEOFF_OP_RESIDUAL_COMPARISON),
        "short_run_handling" => Some(TAKEOFF_OP_SHORT_RUN_HANDLING),
        "selection" => Some(TAKEOFF_OP_CROSSING_SELECTION),
        _ => None,
    }
}

/// The threshold rule, then each operator composed onto it, as separate entries.
pub(crate) fn bound_methods(
    method_id: &str,
    values: BoundValues,
    request: &AnalysisRequest,
    manual_override: bool,
) -> Vec<BoundMethod> {
    crate::resolution::bound_with_operators(
        method_id,
        values,
        operator_for,
        |id| request.is_backed(id),
        manual_override,
    )
}

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
        "takeoff.threshold.landing_shape" => {
            landing_shape::crossing(trial, epoch, *threshold_newtons, resolved, warnings)
        }
        other => Err(RuleRefusal::Refused(Box::new(
            crate::binding::unbound_method_refusal(other, "takeoff"),
        ))),
    }
}

pub(crate) fn resolve(
    trial: &Trial,
    epoch: &WeighingEpoch,
    choice: &MethodChoice,
    warnings: &mut Vec<String>,
) -> TakeoffOutcome {
    let mut resolved = Resolution::over(&choice.parameters, &choice.options, choice.claims());
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
