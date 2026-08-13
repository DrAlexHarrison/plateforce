//! `rfd.peak_sliding_window`: the steepest chord of a stated width anywhere in the analysis
//! window.
//!
//! The width is the whole of the number. Four studies reach four verdicts across a disputed
//! range of 1.67 to 50 ms, and part of that disagreement is not about the traces at all,
//! because one used a 10 percent acceptance criterion and the others 15 percent. The rate
//! rises as the width narrows and with sampling rate and filter cutoff, because the maximum
//! of a noisy series grows with the number of independent samples, so a 2 ms window on a
//! noisy trace measures the filter as much as the athlete.
//!
//! The search runs over the analysis window and never from onset. The entry's claim for this
//! rule is that it is onset-independent, which makes it the natural cross-implementation
//! diagnostic, and a search bounded at onset would take that away.
//!
//! A two-sample centred difference is this rule at a width of two sample intervals, which the
//! entry states and the core holds, so it is a width here rather than a second entry.

use crate::derived::{DerivedContext, DerivedOutcome, DerivedRule};
use crate::request::MethodChoice;
use crate::resolution::{Resolution, RuleRefusal};
use crate::response::Quantity;
use crate::slots::analysis_window;

pub const ID: &str = "rfd.peak_sliding_window";

/// The entry's own name for the width, and the value it publishes as its default.
pub const WIDTH_PARAMETER: &str = "window_width_ms";
pub const WIDTH_DEFAULT_MILLISECONDS: f64 = 20.0;

pub const QUANTITIES: &[Quantity] = &[Quantity {
    key: super::KEY,
    label: "Rate of force development",
    unit: "newtons_per_second",
    computed_by: Some(ID),
    produced_by_construct: None,
}];

pub const RULE: DerivedRule = compute;

fn compute(
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
    let (width_milliseconds, width_samples) = resolved.milliseconds_and_samples(
        WIDTH_PARAMETER,
        WIDTH_DEFAULT_MILLISECONDS,
        context.trial.sample_rate_hz(),
    );
    let bound = resolved.finish();

    let Some((start, end)) = analysis_window::span(context) else {
        return DerivedOutcome::declined(
            bound,
            context.unavailable(ID, &[analysis_window::CONSTRUCT]),
        );
    };

    let Some(steepest) = plateforce_core::rate::steepest_chord(
        context.trial.force(),
        width_samples,
        start,
        end,
        context.trial.sample_interval_seconds(),
    ) else {
        // A width of zero and a width longer than the window both land here, and both are
        // the stated number rather than the recording, so the refusal names the parameter.
        return DerivedOutcome::declined(
            bound,
            RuleRefusal::Refused(Box::new(plateforce_core::Refusal::value_not_accepted(
                ID,
                WIDTH_PARAMETER,
                width_milliseconds,
                vec![format!(
                    "a width above zero that the analysis window holds, which is at most {:.1} ms here",
                    end.saturating_sub(start) as f64 * 1000.0 / context.trial.sample_rate_hz()
                )],
            ))),
        );
    };

    DerivedOutcome {
        values: vec![(super::KEY, Some(steepest.rate_newtons_per_second()))],
        placed: Vec::new(),
        bound,
        refusal: None,
    }
}
