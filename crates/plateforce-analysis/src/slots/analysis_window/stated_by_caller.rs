//! `window.stated.by_caller`: the interval the caller states, as two instants in seconds.
//!
//! A window a reader drags across the trace and a window a script states with two instants are
//! one method reached two ways, so both arrive here and both record the two ends as the
//! caller's own. No rule places either end, which is the whole content of the entry: a window
//! filled from a published constant would put a published rule's name on a boundary the reader
//! chose.
//!
//! Both ends are required and neither is defaulted, so a caller naming this rule without
//! stating them is refused by name rather than handed the recording's own extent.

use crate::derived::{DerivedContext, DerivedOutcome, DerivedRule};
use crate::request::MethodChoice;
use crate::resolution::{Resolution, RuleRefusal};
use crate::response::Quantity;

pub const ID: &str = "window.stated.by_caller";

pub const START_PARAMETER: &str = "start_seconds";
pub const END_PARAMETER: &str = "end_seconds";

/// The same two keys the rules that search for a window report, so a reader comparing a stated
/// window against a placed one holds the key still and watches `computed_by` change.
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
    let stated_start = resolved.required_number(ID, START_PARAMETER);
    let stated_end = resolved.required_number(ID, END_PARAMETER);
    let bound = resolved.finish();

    let (start_seconds, end_seconds) = match (stated_start, stated_end) {
        (Ok(start), Ok(end)) => (start, end),
        (Err(refusal), _) | (_, Err(refusal)) => return DerivedOutcome::declined(bound, refusal),
    };

    let last_sample = context.trial.len().saturating_sub(1);
    let recording = format!(
        "a time from 0.0000 to {:.4} s, the extent of this recording",
        context.trial.time_at(last_sample)
    );
    for (name, seconds) in [
        (START_PARAMETER, start_seconds),
        (END_PARAMETER, end_seconds),
    ] {
        if !seconds.is_finite() {
            return DerivedOutcome::declined(
                bound,
                RuleRefusal::Refused(Box::new(plateforce_core::Refusal::parameter_not_finite(
                    ID, name, seconds,
                ))),
            );
        }
        if seconds < 0.0 || seconds > context.trial.time_at(last_sample) {
            return DerivedOutcome::declined(
                bound,
                RuleRefusal::Refused(Box::new(plateforce_core::Refusal::value_not_accepted(
                    ID,
                    name,
                    seconds,
                    vec![recording.clone()],
                ))),
            );
        }
    }

    let start = sample_at(context, start_seconds).min(last_sample);
    let named_end = sample_at(context, end_seconds).min(last_sample);

    // Two samples is the floor a window has to clear, because an interval narrower than one
    // sampling interval encloses no interval at all. The pair is refused rather than widened:
    // widening would answer a question about a different window from the one that was asked.
    if named_end <= start {
        return DerivedOutcome::declined(
            bound,
            RuleRefusal::Refused(Box::new(plateforce_core::Refusal::span_selects_no_samples(
                ID, start, named_end,
            ))),
        );
    }

    // The core's spans are half open and the caller states two instants that are both inside
    // the window they drew, so the end is one past the sample they named.
    DerivedOutcome {
        values: vec![
            (
                "analysis_window_start_seconds",
                Some(context.trial.time_at(start)),
            ),
            (
                "analysis_window_end_seconds",
                Some(context.trial.time_at(named_end)),
            ),
        ],
        placed: vec![(super::START, start), (super::END, named_end + 1)],
        bound,
        refusal: None,
    }
}

/// The sample nearest a stated instant. Nearest rather than truncated, because a reader
/// dragging to a sample and a script naming that sample's own time must reach the same one.
fn sample_at(context: &DerivedContext, seconds: f64) -> usize {
    (seconds * context.trial.sample_rate_hz()).round().max(0.0) as usize
}
