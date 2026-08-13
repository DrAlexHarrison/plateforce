//! `window_end.fixed_duration.isometric`: a fixed test length measured from onset.
//!
//! The registry's text: the window ends at the sample nearest a fixed test length after
//! onset. Its scope is an isometric test, where the window is the protocol's own duration
//! rather than anything read off the trace, and the entry records that it reads 1.0 N low
//! against the rule that stops where force falls away from its running maximum. That
//! comparison is inference on a corpus holding no isometric trial, and it stays inference
//! until one is recorded.
//!
//! It runs on any recording, and on a jump it returns the two seconds after onset.

use crate::binding::ONSET_CONSTRUCT;
use crate::derived::{DerivedContext, DerivedOutcome, DerivedRule};
use crate::request::MethodChoice;
use crate::resolution::Resolution;
use crate::response::Quantity;

pub const ID: &str = "window_end.fixed_duration.isometric";

/// The published default, in the entry's own name for it.
const LENGTH_PARAMETER: &str = "length_seconds";
const LENGTH_DEFAULT_SECONDS: f64 = 2.0;

/// Both rules for this construct report the same two quantities under the same two keys, so
/// a reader comparing them holds the key still and watches `computed_by` change.
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
    warnings: &mut Vec<String>,
) -> DerivedOutcome {
    let mut resolved = Resolution::over(
        &choice.parameters,
        &choice.options,
        choice.declared.of_entry(ID),
        choice.claims(),
    );
    let length_seconds = resolved.number(LENGTH_PARAMETER, LENGTH_DEFAULT_SECONDS);
    let bound = resolved.finish();

    let Some(onset) = context.onset_index() else {
        return DerivedOutcome::declined(bound, context.unavailable(ID, &[ONSET_CONSTRUCT]));
    };

    let length_samples = (length_seconds * context.trial.sample_rate_hz())
        .round()
        .max(0.0) as usize;
    let named = onset + length_samples;
    let end = (named + 1).min(context.trial.len());

    // The window a reader asked for and the window they got are the same only while the
    // recording is long enough to hold it, and a number taken over a window shortened by the
    // end of the file is not the number the entry describes.
    if named >= context.trial.len() {
        warnings.push(format!(
            "{ID} was asked for {length_seconds} s after onset and the recording ends {:.4} s after it, so this window is the shorter of the two",
            context.trial.time_at(context.trial.len() - 1) - context.trial.time_at(onset)
        ));
    }

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
