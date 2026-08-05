//! `window_end.force_dropoff_from_running_max`: the window ends where a smoothed force falls
//! away from its own running maximum.
//!
//! The registry's text: take a centred rectangular moving average, start at the peak rate of
//! force development, track the running maximum of the average, and stop at the first sample
//! where the average has fallen more than a stated percentage below that maximum. It answers
//! how long an effort was maintained, which is a different question from the two seconds the
//! fixed-length rule takes and from where the jump ended.
//!
//! The running maximum only rises and the stop compares against a fixed fraction of it, so a
//! single high sample raises the level the trace has to fall below and closes the window
//! earlier than the decay alone would, never later. The entry says that, and it is why the
//! percentage travels with the result.
//!
//! The moving average is a second smoother cascaded on whatever conditioning rule already
//! ran, so the effective smoothing on this decision is not the filter setting a reader sees.
//! Its length is a published parameter here rather than a constant, for that reason.

use plateforce_core::phases::window_end_by_force_dropoff_from_running_maximum;
use plateforce_core::statistics::DurationRounding;

use crate::binding::ONSET_CONSTRUCT;
use crate::derived::{DerivedContext, DerivedOutcome, DerivedRule};
use crate::request::MethodChoice;
use crate::resolution::{Resolution, RuleRefusal};
use crate::response::Quantity;

pub const ID: &str = "window_end.force_dropoff_from_running_max";

/// The entry's own names for its two parameters, and the values it publishes for them.
pub const DROPOFF_PARAMETER: &str = "dropoff_pct";
const DROPOFF_DEFAULT_PERCENT: f64 = 5.0;
pub const MOVING_AVERAGE_PARAMETER: &str = "moving_average_seconds";
const MOVING_AVERAGE_DEFAULT_SECONDS: f64 = 0.1;

/// The width, in sample intervals, of the difference whose maximum anchors the search.
///
/// The rule starts at the peak rate of force development and states no width for it. A
/// two-sample centred difference is the steepest-chord rule at a width of two sample
/// intervals, which the rate entry states and the core holds, so the anchor is read through
/// that one function rather than through a second spelling of a derivative. Reading it at any
/// other width would put a knob on this rule that its entry does not publish.
const ANCHOR_DIFFERENCE_WIDTH_SAMPLES: usize = 2;

/// The three rules for this construct report the same two quantities under the same two keys,
/// so a reader comparing them holds the key still and watches `computed_by` change.
pub const QUANTITIES: &[Quantity] = &[
    Quantity {
        key: "analysis_window_start_seconds",
        label: "Analysis window, start",
        unit: "seconds",
        computed_by: Some(ID),
    },
    Quantity {
        key: "analysis_window_end_seconds",
        label: "Analysis window, end",
        unit: "seconds",
        computed_by: Some(ID),
    },
];

pub const RULE: DerivedRule = place;

fn place(
    context: &DerivedContext,
    choice: &MethodChoice,
    _warnings: &mut Vec<String>,
) -> DerivedOutcome {
    let mut resolved = Resolution::over(&choice.parameters, &choice.options, choice.claims());
    let dropoff_percent = resolved.number(DROPOFF_PARAMETER, DROPOFF_DEFAULT_PERCENT);
    let moving_average_seconds =
        resolved.number(MOVING_AVERAGE_PARAMETER, MOVING_AVERAGE_DEFAULT_SECONDS);

    let Some(onset) = context.onset_index() else {
        return DerivedOutcome::declined(
            resolved.finish(),
            context.unavailable(ID, &[ONSET_CONSTRUCT]),
        );
    };

    let force = context.trial.force();
    let last = force.len().saturating_sub(1);
    let anchor = plateforce_core::rate::steepest_chord(
        force,
        ANCHOR_DIFFERENCE_WIDTH_SAMPLES,
        onset,
        last,
        context.trial.sample_interval_seconds(),
    )
    // The chord spans two intervals either side of the sample it is centred on, so the
    // instant the rate is largest at is the middle of the span rather than its start.
    .map(|steepest| steepest.start_index + ANCHOR_DIFFERENCE_WIDTH_SAMPLES / 2);

    let Some(anchor) = anchor else {
        return DerivedOutcome::declined(
            resolved.finish(),
            RuleRefusal::Refused(Box::new(plateforce_core::Refusal::nothing_qualified(
                ID,
                last.saturating_sub(onset),
                std::collections::BTreeMap::from([
                    (
                        "search_start_seconds".to_string(),
                        context.trial.time_at(onset),
                    ),
                    ("search_end_seconds".to_string(), context.trial.time_at(last)),
                ]),
            ))),
        );
    };

    let stopped = window_end_by_force_dropoff_from_running_maximum(
        force,
        moving_average_seconds,
        context.trial.sample_rate_hz(),
        DurationRounding::Nearest,
        dropoff_percent,
        anchor,
    );
    let bound = resolved.finish();

    let end = match stopped {
        // A trace that never falls that far was held to the end of the recording, which is
        // the answer the rule gives rather than an absence: the effort outlasted the file.
        Ok(None) => force.len(),
        Ok(Some(index)) => (index + 1).min(force.len()),
        // The smoother declining is the moving average not fitting the recording, which is a
        // fact about the two lengths rather than about the force in between them.
        Err(_) => {
            return DerivedOutcome::declined(
                bound,
                RuleRefusal::Refused(Box::new(plateforce_core::Refusal::epoch_does_not_fit(
                    ID,
                    moving_average_seconds,
                    context.trial.time_at(onset),
                    context.trial.time_at(last),
                ))),
            )
        }
    };

    DerivedOutcome {
        values: vec![
            (
                "analysis_window_start_seconds",
                Some(context.trial.time_at(onset)),
            ),
            (
                "analysis_window_end_seconds",
                Some(context.trial.time_at(end.saturating_sub(1))),
            ),
        ],
        placed: vec![(super::START, onset), (super::END, end)],
        bound,
        refusal: None,
    }
}
