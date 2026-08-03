//! `force.peak.gross`: the maximum of the force series, system weight included.

use crate::derived::{DerivedContext, DerivedOutcome, DerivedRule};
use crate::request::MethodChoice;
use crate::resolution::{Resolution, RuleRefusal};
use crate::response::Quantity;
use crate::slots::analysis_window;

pub const ID: &str = "force.peak.gross";

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
    let resolved = Resolution::over(
        &choice.parameters,
        &choice.options,
        &choice.recommended,
        &choice.from_registry_default,
    );
    let bound = resolved.finish();

    let Some((start, end)) = analysis_window::span(context) else {
        return DerivedOutcome::declined(
            bound,
            context.unavailable(ID, &[analysis_window::CONSTRUCT]),
        );
    };

    // The maximum declines on one thing only, a span selecting no samples, and both of that
    // span's ends are the numbers a caller moves to fix it.
    match plateforce_core::peak::maximum_over(context.trial.force(), start, end) {
        Ok(peak) => DerivedOutcome {
            values: vec![(super::KEY, Some(peak))],
            placed: Vec::new(),
            bound,
            refusal: None,
        },
        Err(_) => DerivedOutcome::declined(
            bound,
            RuleRefusal::Refused(Box::new(plateforce_core::Refusal::span_selects_no_samples(
                ID, start, end,
            ))),
        ),
    }
}
