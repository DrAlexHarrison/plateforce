//! Where the athlete leaves the plate, and the threshold every later comparison runs
//! against.

pub mod absolute_force;
pub mod descending_crossing;
pub mod flight_noise_k_sd;
pub mod landing_shape;
pub mod longest_run;

use plateforce_core::provenance::ParameterSource;
use plateforce_core::{Trial, WeighingEpoch};

use crate::derived::WEIGHING_EPOCH;
use crate::request::{AnalysisRequest, MethodChoice};
use crate::resolution::{BoundMethod, BoundValues, Resolution, RuleRefusal};

// Written once and read from here by everything that names one, for the same reason the
// onset family is: a list and a match spelling the same id are free to drift apart.
pub(crate) const TAKEOFF_OP_CROSSING_SELECTION: &str = "takeoff.op.crossing_selection";
pub(crate) const TAKEOFF_OP_SHORT_RUN_HANDLING: &str = "takeoff.op.short_run_handling";
pub(crate) const TAKEOFF_OP_RESIDUAL_COMPARISON: &str = "takeoff.op.residual_comparison";
/// A floor the weighing rule settled, which no caller chose and which moves with it.
pub const TAKEOFF_SEARCH_FLOOR_AT_WEIGHING_EPOCH_END: &str =
    "takeoff.op.search_floor_at_weighing_epoch_end";
/// The policy of considering every sample, which the other three shipped rules take.
pub const TAKEOFF_SEARCH_FLOOR_AT_TRIAL_START: &str = "takeoff.op.search_floor_at_trial_start";

/// The names those two operators carry their values under. Named once because the rule that
/// records a value and the reader that looks it up again are in different files.
pub const TAKEOFF_WEIGHING_EPOCH_END_SECONDS: &str = "weighing_epoch_end_seconds";
pub const TAKEOFF_SEARCH_FLOOR_SECONDS: &str = "search_floor_seconds";

/// The entries this build composes onto a takeoff threshold rule. Each is a registry entry
/// in its own right, filed under the takeoff construct rather than the onset one.
pub const TAKEOFF_OPERATOR_IDS: &[&str] = &[
    TAKEOFF_OP_CROSSING_SELECTION,
    TAKEOFF_OP_RESIDUAL_COMPARISON,
    TAKEOFF_OP_SHORT_RUN_HANDLING,
    TAKEOFF_SEARCH_FLOOR_AT_TRIAL_START,
    TAKEOFF_SEARCH_FLOOR_AT_WEIGHING_EPOCH_END,
];

/// The first sample a rule flooring at the weighing window may examine, written into the
/// record beside the takeoff it produced.
///
/// Recorded whether or not the takeoff lands on it, because a floor stated only when it binds
/// tells a reader nothing about the runs where it did not.
pub(crate) fn record_search_floor_at_weighing_epoch_end(
    trial: &Trial,
    epoch: &WeighingEpoch,
    resolved: &mut Resolution,
) {
    let seconds = trial.time_at(epoch.end_index);
    resolved.record_measured(
        TAKEOFF_WEIGHING_EPOCH_END_SECONDS,
        seconds,
        format!("{seconds:.4}"),
        ParameterSource::Measured,
    );
}

/// The same fact for the three rules that forbid nothing, so the two policies read as two
/// values rather than as one value and a silence.
pub(crate) fn record_search_floor_at_trial_start(trial: &Trial, resolved: &mut Resolution) {
    let seconds = trial.time_at(0);
    resolved.record_measured(
        TAKEOFF_SEARCH_FLOOR_SECONDS,
        seconds,
        format!("{seconds:.4}"),
        ParameterSource::Assumed,
    );
}

/// Which registry entry carries each name a takeoff rule reads.
///
/// The threshold rule carries its threshold and its persistence span, which its entry
/// publishes. The rest are operators with entries of their own: `comparison` and
/// `short_run_handling` decide whether an unloaded plate reading negative counts as flight
/// and whether a run too short to be a flight can win the comparison and disqualify the
/// trial, and a threshold row lists neither.
fn operator_for(name: &str) -> Option<&'static str> {
    match name {
        "comparison" => Some(TAKEOFF_OP_RESIDUAL_COMPARISON),
        "short_run_handling" => Some(TAKEOFF_OP_SHORT_RUN_HANDLING),
        "selection" => Some(TAKEOFF_OP_CROSSING_SELECTION),
        TAKEOFF_WEIGHING_EPOCH_END_SECONDS => Some(TAKEOFF_SEARCH_FLOOR_AT_WEIGHING_EPOCH_END),
        TAKEOFF_SEARCH_FLOOR_SECONDS => Some(TAKEOFF_SEARCH_FLOOR_AT_TRIAL_START),
        _ => None,
    }
}

/// The threshold rule, then each operator composed onto it, as separate entries.
pub(crate) fn bound_methods(
    method_id: &str,
    values: BoundValues,
    request: &AnalysisRequest,
    placed_by_hand_at_sample: Option<usize>,
) -> Vec<BoundMethod> {
    crate::resolution::bound_with_operators(
        method_id,
        values,
        operator_for,
        |id| request.is_backed(id),
        placed_by_hand_at_sample,
    )
}

pub(crate) struct TakeoffOutcome {
    pub index: Option<usize>,
    pub threshold_newtons: f64,
    pub bound: BoundValues,
    pub refusal: Option<RuleRefusal>,
}

/// Which of the analysis's own landmarks this rule reads, or nothing where this build files
/// no takeoff rule under the id.
///
/// One arm per arm of `crossing` below, and each answer is that arm's argument list. Two of
/// the five are handed no epoch: `longest_run` ranks every low-force run in the recording and
/// `descending_crossing` confirms a crossing against the residual threshold alone, so neither
/// rests on the weighing rule and neither names it. The other three do, two for the search
/// floor at the epoch's end and `landing_shape` for the system weight its landing peak is
/// measured in.
///
/// No takeoff rule reads the onset. Takeoff settles first, because one onset rule searches
/// back from a point only takeoff bounds.
///
/// `None` for an unknown id rather than an empty list, so a rule added with no arm here is
/// caught as an unanswered question instead of reading as a rule that rests on nothing.
pub(crate) fn landmarks_read(method_id: &str) -> Option<&'static [&'static str]> {
    match method_id {
        "takeoff.threshold.longest_run" => Some(&[]),
        "takeoff.threshold.descending_crossing" => Some(&[]),
        "takeoff.threshold.flight_noise_k_sd" => Some(&[WEIGHING_EPOCH]),
        "takeoff.threshold.absolute_force" => Some(&[WEIGHING_EPOCH]),
        "takeoff.threshold.landing_shape" => Some(&[WEIGHING_EPOCH]),
        _ => None,
    }
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
