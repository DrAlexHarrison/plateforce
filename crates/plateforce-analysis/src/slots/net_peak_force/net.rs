//! `force.peak.net`: the maximum after system weight has been subtracted.
//!
//! Subtracting a constant commutes with taking a maximum, so the peak of the net series and
//! the net of the peak are one number and this rule takes the second route. The other
//! reading in the literature, peak less the force at onset, makes peak force move when the
//! onset rule moves; that is a different rule and would be a different entry.

use crate::derived::{DerivedContext, DerivedOutcome, DerivedRule};
use crate::request::MethodChoice;
use crate::resolution::{Resolution, RuleRefusal};
use crate::response::Quantity;
use crate::slots::analysis_window;

pub const ID: &str = "force.peak.net";

pub const QUANTITIES: &[Quantity] = &[Quantity {
    key: super::KEY,
    label: "Peak force, net",
    unit: "newtons",
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

    let Some((start, end)) = analysis_window::span(context) else {
        return DerivedOutcome::declined(
            bound,
            context.unavailable(ID, &[analysis_window::CONSTRUCT]),
        );
    };

    match plateforce_core::peak::maximum_over(context.trial.force(), start, end) {
        Ok(peak) => DerivedOutcome {
            values: vec![(
                super::KEY,
                Some(peak - context.epoch().system_weight_newtons),
            )],
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
