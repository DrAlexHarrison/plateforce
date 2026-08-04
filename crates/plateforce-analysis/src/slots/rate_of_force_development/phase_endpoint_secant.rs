//! `rfd.phase_endpoint_secant.harry`: the change in force across a declared phase divided by
//! the time between its two ends.
//!
//! A secant rather than a derivative, so it understates the peak instantaneous rate by an
//! amount that grows with phase duration and with how non-linear the rise is, and it inherits
//! the phase-boundary disagreement at both ends rather than at one.
//!
//! The phase is the one the propulsion rules declared, read from the boundaries they placed.
//! Nothing here picks an interval: the number is taken across whatever those two rules put on
//! the trace, and the chain behind it names both of them, so a reader sees which propulsion
//! start and which propulsion end produced the rate.

use crate::derived::{DerivedContext, DerivedOutcome, DerivedRule};
use crate::request::MethodChoice;
use crate::resolution::{Resolution, RuleRefusal};
use crate::response::Quantity;

pub const ID: &str = "rfd.phase_endpoint_secant.harry";

pub const QUANTITIES: &[Quantity] = &[Quantity {
    key: super::KEY,
    label: "Rate of force development",
    unit: "newtons_per_second",
    computed_by: Some(ID),
}];

pub const RULE: DerivedRule = compute;

fn compute(
    context: &DerivedContext,
    choice: &MethodChoice,
    _warnings: &mut Vec<String>,
) -> DerivedOutcome {
    let resolved = Resolution::over(&choice.parameters, &choice.options, choice.claims());
    let bound = resolved.finish();

    let (start, end) = match super::propulsion_interval(context) {
        Ok(interval) => interval,
        Err(missing) => return DerivedOutcome::declined(bound, context.unavailable(ID, &missing)),
    };

    let Some(chord) = plateforce_core::rate::chord(
        context.trial.force(),
        start,
        end,
        context.trial.sample_interval_seconds(),
    ) else {
        return DerivedOutcome::declined(
            bound,
            RuleRefusal::Refused(Box::new(plateforce_core::Refusal::span_selects_no_samples(
                ID, start, end,
            ))),
        );
    };

    DerivedOutcome {
        values: vec![(super::KEY, Some(chord.rate_newtons_per_second()))],
        placed: Vec::new(),
        bound,
        refusal: None,
    }
}
