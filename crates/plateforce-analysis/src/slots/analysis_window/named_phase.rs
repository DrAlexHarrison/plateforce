//! `window.from_named_phase`: the window is the phase interval the caller names.
//!
//! The boundaries are not this rule's. They belong to whichever rules are bound to the two
//! constructs the named phase runs between, so a number taken over this window rests on those
//! rules and the chain says so without this entry naming any of them.
//!
//! That is the whole difference between this and `window.stated.by_caller`. Two windows can
//! enclose the same samples and mean different things: one is where a reader looked, the other
//! is where a published rule placed a boundary, and a record that could not tell them apart
//! would reproduce in this software the defect it was built to expose.
//!
//! The phase is required and undefaulted. Each value names the two boundaries it runs between
//! rather than a school's word for the interval, because which named phase those boundaries
//! enclose is settled by the rules bound to them.

use crate::boundaries;
use crate::derived::{DerivedContext, DerivedOutcome, DerivedRule};
use crate::request::MethodChoice;
use crate::resolution::Resolution;
use crate::response::Quantity;

pub const ID: &str = "window.from_named_phase";

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
    let mut resolved = Resolution::over(
        &choice.parameters,
        &choice.options,
        choice.declared.of_entry(ID),
        choice.claims(),
    );
    let named =
        resolved.required_enumerated(ID, boundaries::PHASE_PARAMETER, boundaries::PHASE_VALUES);
    let bound = resolved.finish();

    let phase = match named {
        Ok(phase) => phase,
        Err(refusal) => return DerivedOutcome::declined(bound, refusal),
    };

    match boundaries::phase_interval(context, phase) {
        Ok((first, last)) => DerivedOutcome {
            values: vec![
                (
                    "analysis_window_start_seconds",
                    Some(context.trial.time_at(first)),
                ),
                (
                    "analysis_window_end_seconds",
                    Some(context.trial.time_at(last)),
                ),
            ],
            placed: vec![(super::START, first), (super::END, last + 1)],
            bound,
            refusal: None,
        },
        Err(missing) => DerivedOutcome::declined(bound, context.unavailable(ID, &missing)),
    }
}
