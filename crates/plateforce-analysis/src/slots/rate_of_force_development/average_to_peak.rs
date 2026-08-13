//! `rfd.average_to_peak_force`: peak force divided by the time from onset to the instant of
//! peak force.
//!
//! Deprecated in the registry, and shipped so that years of numbers carrying this label stay
//! interpretable. The source that files it also warns against it: it has the lowest
//! reliability of any variable in the isometric domain, ICC 0.74, because peak force occurs
//! at wildly different times between trials, days and individuals, so the denominator carries
//! variance unrelated to the athlete's rapid-force capability.
//!
//! It is a separate rule from the epoch family rather than a parameter of it, because the
//! epoch family anchors its second point at a fixed time and this anchors it at an event the
//! data decides.

use crate::binding::ONSET_CONSTRUCT;
use crate::derived::{DerivedContext, DerivedOutcome, DerivedRule};
use crate::request::MethodChoice;
use crate::resolution::{Resolution, RuleRefusal};
use crate::response::Quantity;
use crate::slots::analysis_window;

pub const ID: &str = "rfd.average_to_peak_force";

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
    let resolved = Resolution::over(
        &choice.parameters,
        &choice.options,
        choice.declared.of_entry(ID),
        choice.claims(),
    );
    let bound = resolved.finish();

    let onset = context.onset_index();
    let window = analysis_window::span(context);
    let (Some(onset), Some((start, end))) = (onset, window) else {
        let mut missing = Vec::new();
        if onset.is_none() {
            missing.push(ONSET_CONSTRUCT);
        }
        if window.is_none() {
            missing.push(analysis_window::CONSTRUCT);
        }
        return DerivedOutcome::declined(bound, context.unavailable(ID, &missing));
    };

    let force = context.trial.force();
    let Ok(peak_index) = plateforce_core::peak::index_of_maximum_over(force, start, end) else {
        return DerivedOutcome::declined(
            bound,
            RuleRefusal::Refused(Box::new(plateforce_core::Refusal::span_selects_no_samples(
                ID, start, end,
            ))),
        );
    };

    // A peak at or before onset leaves no interval to divide by. It is the window and the
    // onset that put it there, so the refusal carries both samples rather than a parameter
    // this rule does not have.
    if peak_index <= onset {
        return DerivedOutcome::declined(
            bound,
            RuleRefusal::Refused(Box::new(plateforce_core::Refusal::span_selects_no_samples(
                ID, onset, peak_index,
            ))),
        );
    }

    let elapsed_seconds = (peak_index - onset) as f64 * context.trial.sample_interval_seconds();
    DerivedOutcome {
        values: vec![(super::KEY, Some(force[peak_index] / elapsed_seconds))],
        placed: Vec::new(),
        bound,
        refusal: None,
    }
}
