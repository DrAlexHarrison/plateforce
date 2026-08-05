//! `force.peak.estimator`: the maximum of the raw series, or of a centred moving average of
//! stated width.
//!
//! The entry publishes `averaging_window_seconds = 0`, which is the raw maximum, so this
//! rule and `force.peak.gross` agree until somebody states a width. Above zero they separate
//! in one direction only.

use crate::derived::{DerivedContext, DerivedOutcome, DerivedRule};
use crate::request::MethodChoice;
use crate::resolution::{Resolution, RuleRefusal};
use crate::response::Quantity;
use crate::slots::analysis_window;

pub const ID: &str = "force.peak.estimator";

/// The entry's own name for the width, and the width it publishes.
pub const WINDOW_PARAMETER: &str = "averaging_window_seconds";
pub const WINDOW_DEFAULT_SECONDS: f64 = 0.0;

pub const QUANTITIES: &[Quantity] = &[Quantity {
    key: super::KEY,
    label: "Peak force",
    unit: "newtons",
    computed_by: Some(ID),
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
    let window_seconds = resolved.number(WINDOW_PARAMETER, WINDOW_DEFAULT_SECONDS);
    let bound = resolved.finish();

    let Some((start, end)) = analysis_window::span(context) else {
        return DerivedOutcome::declined(
            bound,
            context.unavailable(ID, &[analysis_window::CONSTRUCT]),
        );
    };

    let window_samples = (window_seconds * context.trial.sample_rate_hz())
        .round()
        .max(0.0) as usize;
    // The average is taken over the whole recording and the maximum over the window. The
    // other order gives the window's first and last samples an edge fit computed from a
    // stretch the recording does not end at, which moves the peak by the width of the
    // window and refuses outright whenever the window is the shorter of the two.
    match plateforce_core::peak::maximum_of_moving_average_over(
        context.trial.force(),
        start,
        end,
        window_samples,
    ) {
        Ok(peak) => DerivedOutcome {
            values: vec![(super::KEY, Some(peak))],
            placed: Vec::new(),
            bound,
            refusal: None,
        },
        Err(plateforce_core::peak::PeakError::EmptySpan { .. }) => DerivedOutcome::declined(
            bound,
            RuleRefusal::Refused(Box::new(plateforce_core::Refusal::span_selects_no_samples(
                ID, start, end,
            ))),
        ),
        // A width longer than the recording is a number the caller stated and can restate,
        // so it comes back naming the parameter and the value rather than the span.
        Err(plateforce_core::peak::PeakError::Smoothing(_)) => DerivedOutcome::declined(
            bound,
            RuleRefusal::Refused(Box::new(plateforce_core::Refusal::value_not_accepted(
                ID,
                WINDOW_PARAMETER,
                window_seconds,
                vec![format!(
                    "a width the recording holds, which is at most {:.4} s here",
                    context.trial.duration_seconds()
                )],
            ))),
        ),
    }
}
