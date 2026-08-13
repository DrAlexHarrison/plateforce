//! `window_end.takeoff.detected`: the recording up to the sample takeoff was placed at.
//!
//! The registry's text: the window runs from the first sample of the recording to the sample
//! the bound takeoff rule placed, so an extremum over it is taken over the jump and the
//! landing that follows takeoff falls outside it.
//!
//! Which sample takeoff is placed at is the takeoff rule's answer and not this rule's, so
//! this window moves when that rule changes.

use crate::binding::TAKEOFF_CONSTRUCT;
use crate::derived::{DerivedContext, DerivedOutcome, DerivedRule};
use crate::request::MethodChoice;
use crate::resolution::Resolution;
use crate::response::Quantity;

pub const ID: &str = "window_end.takeoff.detected";

/// The window's ends as times, so a reader sees the stretch a number was taken over rather
/// than inferring it from two sample counts.
pub const QUANTITIES: &[Quantity] = &[
    Quantity {
        key: "analysis_window_start_seconds",
        label: "Analysis window, start",
        unit: "seconds",
        computed_by: Some(ID),
        produced_by_construct: None,
    },
    Quantity {
        key: "analysis_window_end_seconds",
        label: "Analysis window, end",
        unit: "seconds",
        computed_by: Some(ID),
        produced_by_construct: None,
    },
];

pub const RULE: DerivedRule = place;

fn place(
    context: &DerivedContext,
    choice: &MethodChoice,
    _warnings: &mut Vec<String>,
) -> DerivedOutcome {
    let resolved = Resolution::over(
        &choice.parameters,
        &choice.options,
        choice.declared.of_entry(ID),
        choice.claims(),
    );
    let bound = resolved.finish();

    let Some(takeoff) = context.takeoff_index() else {
        return DerivedOutcome::declined(bound, context.unavailable(ID, &[TAKEOFF_CONSTRUCT]));
    };

    // The sample takeoff was placed at is inside the window the entry describes, and the
    // core's spans are half open, so the end is one past it. Off by one here is one sample
    // of the propulsive peak, which is where the largest force in the recording sits.
    let start = 0usize;
    let end = (takeoff + 1).min(context.trial.len());

    DerivedOutcome {
        values: vec![
            (
                "analysis_window_start_seconds",
                Some(context.trial.time_at(start)),
            ),
            (
                "analysis_window_end_seconds",
                Some(context.trial.time_at(end.saturating_sub(1))),
            ),
        ],
        placed: vec![(super::START, start), (super::END, end)],
        bound,
        refusal: None,
    }
}
