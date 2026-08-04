//! Where the jump starts. Net impulse reliability runs from 0.984 to 0.479 across published
//! rules on identical data, which is why this slot's every operator is recorded.

pub mod absolute_force;
pub mod adaptive_trailing_window;
pub mod last_within_band;
pub mod noise_relative;
pub mod relative_to_system_weight;

use plateforce_core::provenance::ParameterSource;

use plateforce_core::onset::{backtrack, CrossingSearch, CrossingSelection};
use plateforce_core::{Trial, WeighingEpoch};

use crate::request::{AnalysisRequest, MethodChoice};
use crate::resolution::{format_number, BoundMethod, BoundValues, Resolution, RuleRefusal};

/// Which sign of departure from the reference counts as onset. `above_only` is a genuine
/// fork for a squat jump or an isometric pull, which only rise, and is refused here rather
/// than mapped onto a neighbour.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum OnsetDirection {
    BelowOnly,
    TwoSided,
}

/// `onset.op.direction`, in the vocabulary the registry publishes for it. A
/// countermovement jump always unweights first, and counting a departure in either
/// direction lets the upward excursion of rising onto the toes register as onset: net
/// impulse ICC 0.479 either-direction against 0.790 below-only.
///
/// `above_only` is a published value of this operator and is declined here rather than
/// treated as a misspelling, and both report under one code carrying the name that was
/// asked for. Which of the two it was is the difference between a typo and a method fork,
/// and `onset.op.direction`'s own entry is where that difference is written.
pub(crate) fn direction(resolved: &mut Resolution) -> Result<OnsetDirection, RuleRefusal> {
    let chosen = resolved.option("direction", "below_only");
    match chosen.as_str() {
        "below_only" => Ok(OnsetDirection::BelowOnly),
        "two_sided" => Ok(OnsetDirection::TwoSided),
        _ => Err(RuleRefusal::Refused(Box::new(
            plateforce_core::Refusal::name_not_accepted(
                "onset.op.direction",
                "direction",
                chosen,
                vec!["below_only".to_string(), "two_sided".to_string()],
            ),
        ))),
    }
}

/// The registry files both names on the onset row, and in this build the weighing window
/// settles both: the spread the threshold is scaled by is that window's, taken over quiet
/// stance. A rule that read them from its own request would report a convention that did
/// not produce the number.
pub(crate) fn record_inherited_spread(resolved: &mut Resolution, inherited_spread: (&str, bool)) {
    let (convention, stated) = inherited_spread;
    resolved.record(
        "sd_convention",
        convention.to_string(),
        if stated {
            ParameterSource::Stated
        } else {
            ParameterSource::Assumed
        },
    );
    resolved.record(
        "reference_distribution",
        "quiet_stance_force".into(),
        ParameterSource::Assumed,
    );
}

pub(crate) fn onset_search(
    trial: &Trial,
    epoch: &WeighingEpoch,
    resolved: &mut Resolution,
) -> Result<CrossingSearch, RuleRefusal> {
    let rate = trial.sample_rate_hz();
    // Two different operators, and which one ran decides what the record may call it. A
    // stated time is the deprecated fixed floor. Unstated, the search starts where the
    // weighing window ended, which the weighing rule decided and no caller chose, so it is
    // recorded as the derived bound it is rather than as a time nobody stated.
    let start_index = match resolved.stated(FLOOR_SECONDS) {
        Some(seconds) => {
            resolved.record_measured(
                FLOOR_SECONDS,
                seconds,
                format_number(seconds),
                ParameterSource::Stated,
            );
            (seconds * rate).round() as usize
        }
        None => {
            resolved.record_measured(
                WEIGHING_EPOCH_END_SECONDS,
                trial.time_at(epoch.end_index),
                format!("{:.4}", trial.time_at(epoch.end_index)),
                ParameterSource::Measured,
            );
            epoch.end_index
        }
    };
    Ok(CrossingSearch {
        start_index,
        end_index: trial.len(),
        persistence_samples: resolved
            .milliseconds_as_samples("span_ms", 30.0, rate)
            .max(1),
        selection: resolved.enumerated(
            "selection",
            "first",
            &[
                ("first", CrossingSelection::First),
                ("last", CrossingSelection::Last),
            ],
        )?,
    })
}

pub(crate) struct OnsetOutcome {
    pub index: Option<usize>,
    pub bound: BoundValues,
    pub refusal: Option<RuleRefusal>,
}

/// Which core function the onset id names, and what it was given.
fn crossing(
    trial: &Trial,
    epoch: &WeighingEpoch,
    takeoff_index: Option<usize>,
    choice: &MethodChoice,
    inherited_spread: (&str, bool),
    resolved: &mut Resolution,
    warnings: &mut Vec<String>,
) -> Result<usize, RuleRefusal> {
    match choice.method_id.as_str() {
        "onset.threshold.relative_to_system_weight" => {
            relative_to_system_weight::crossing(trial, epoch, resolved)
        }
        "onset.threshold.absolute_force" => absolute_force::crossing(trial, epoch, resolved),
        "onset.threshold.last_within_band" => last_within_band::crossing(
            trial,
            epoch,
            takeoff_index,
            inherited_spread,
            resolved,
            warnings,
        ),
        "onset.threshold.adaptive_trailing_window" => {
            adaptive_trailing_window::crossing(trial, resolved)
        }
        "onset.threshold.noise_relative" => {
            noise_relative::crossing(trial, epoch, inherited_spread, resolved)
        }
        other => Err(RuleRefusal::Refused(Box::new(
            crate::binding::unbound_method_refusal(other, "onset"),
        ))),
    }
}

/// Whether the backward-offset operator is composed onto this rule here. The trailing-window
/// and last-within-band rules resolve their own backtrack.
fn applies_backtrack(method_id: &str) -> bool {
    match method_id {
        "onset.threshold.noise_relative" => noise_relative::APPLIES_BACKTRACK,
        "onset.threshold.relative_to_system_weight" => relative_to_system_weight::APPLIES_BACKTRACK,
        "onset.threshold.absolute_force" => absolute_force::APPLIES_BACKTRACK,
        "onset.threshold.last_within_band" => last_within_band::APPLIES_BACKTRACK,
        "onset.threshold.adaptive_trailing_window" => adaptive_trailing_window::APPLIES_BACKTRACK,
        _ => false,
    }
}

/// `takeoff_index` is what the takeoff slot settled, because one onset rule searches back
/// from a landmark that only exists once the jump's end is known.
pub(crate) fn resolve(
    trial: &Trial,
    epoch: &WeighingEpoch,
    takeoff_index: Option<usize>,
    choice: &MethodChoice,
    inherited_spread: (&str, bool),
    warnings: &mut Vec<String>,
) -> OnsetOutcome {
    let rate = trial.sample_rate_hz();
    let mut resolved = Resolution::over(&choice.parameters, &choice.options, choice.claims());
    let found = crossing(
        trial,
        epoch,
        takeoff_index,
        choice,
        inherited_spread,
        &mut resolved,
        warnings,
    );

    let mut refusal = None;
    let index = match found {
        Ok(index) => {
            if applies_backtrack(&choice.method_id) {
                let back_offset_samples =
                    resolved.milliseconds_as_samples(OFFSET_MILLISECONDS, 30.0, rate);
                let outcome = backtrack(index, back_offset_samples);
                if outcome.clamped_at_start {
                    warnings.push(
                        "the onset backtrack ran off the front of the recording and was clamped at sample zero".into(),
                    );
                }
                Some(outcome.index)
            } else {
                Some(index)
            }
        }
        Err(rejected) => {
            warnings.push(rejected.to_string());
            refusal = Some(rejected);
            None
        }
    };

    OnsetOutcome {
        index,
        bound: resolved.finish(),
        refusal,
    }
}

/// The step back from the crossing, composed onto every onset rule that does not resolve
/// its own.
pub const BACKWARD_OFFSET_FIXED: &str = "onset.op.backward_offset_fixed";
/// A floor the caller stated in seconds.
pub const SEARCH_FLOOR: &str = "onset.op.search_floor";
/// A floor the weighing rule settled and no caller chose.
pub const SEARCH_FLOOR_AT_WEIGHING_EPOCH_END: &str = "onset.op.search_floor_at_weighing_epoch_end";
/// The landmark a backward search stops at, which is what makes a last-crossing rule safe.
pub const SEARCH_UPPER_BOUND: &str = "onset.op.search_upper_bound";
/// The retreat from the crossing to where force returned within a tolerance, and the window
/// it looks back over for the excursion that triggers it.
pub const BACKTRACK_TO_TOLERANCE: &str = "onset.op.backtrack_to_tolerance";

/// The names those operators carry their values under. Named once because the rule that
/// records a value and the reader that looks it up again are in different files, and a name
/// that drifted between them would read as an operator that never ran.
pub const OFFSET_MILLISECONDS: &str = "offset_ms";
pub const FLOOR_SECONDS: &str = "floor_seconds";
pub const WEIGHING_EPOCH_END_SECONDS: &str = "weighing_epoch_end_seconds";
/// Read in seconds, and it says so, because every other span an onset rule takes is stated
/// in milliseconds. A caller writing 500 for this one meaning milliseconds would get a
/// lookback longer than most recordings and no rule would have anything to object to.
pub const INVERSE_LOOKBACK_SECONDS: &str = "inverse_lookback_seconds";

/// The entries this build composes onto an onset threshold rule. Each is a registry entry
/// in its own right, with its own citation, default and published values.
pub const ONSET_OPERATOR_IDS: &[&str] = &[
    BACKTRACK_TO_TOLERANCE,
    BACKWARD_OFFSET_FIXED,
    "onset.op.crossing_selection",
    "onset.op.direction",
    "onset.op.persistence",
    SEARCH_FLOOR,
    SEARCH_FLOOR_AT_WEIGHING_EPOCH_END,
    SEARCH_UPPER_BOUND,
];

/// Which registry entry carries each name an onset rule reads.
///
/// A threshold rule carries its own threshold and the convention its spread was taken
/// under. Every other value is an operator the registry files as an entry in its own right,
/// so recording one against the threshold rule puts a parameter on a row that does not have
/// it, and a reader who looks the id up does not find the value that moved the number.
fn operator_for(name: &str) -> Option<&'static str> {
    match name {
        OFFSET_MILLISECONDS => Some(BACKWARD_OFFSET_FIXED),
        "span_ms" => Some("onset.op.persistence"),
        FLOOR_SECONDS => Some(SEARCH_FLOOR),
        WEIGHING_EPOCH_END_SECONDS => Some(SEARCH_FLOOR_AT_WEIGHING_EPOCH_END),
        "direction" => Some("onset.op.direction"),
        "selection" => Some("onset.op.crossing_selection"),
        // The window searched for an excursion the other side of the band, which is the
        // trigger the retreat fires on.
        INVERSE_LOOKBACK_SECONDS => Some(BACKTRACK_TO_TOLERANCE),
        // The landmark the search stops at, and where on this trace it landed.
        "bound" | "search_bound_seconds" => Some(SEARCH_UPPER_BOUND),
        _ => None,
    }
}

/// The threshold rule, then each operator composed onto it, as separate entries.
///
/// `onset.op.backward_offset_fixed` is the one the registry is most explicit about: it is
/// an entry with its own citation and its own default, its notes say omitting it "is not
/// choosing 0 ms, it is failing to implement the cited method", and a composition that
/// hides it inside the threshold rule reports neither.
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
