//! `rfd.phase_endpoint_secant.harry`: the change in force across a declared phase divided by
//! the time between its two ends.
//!
//! A secant rather than a derivative, so it understates the peak instantaneous rate by an
//! amount that grows with phase duration and with how non-linear the rise is, and it inherits
//! the phase-boundary disagreement at both ends rather than at one.
//!
//! The caller states which phase, and the number is taken across whatever the two rules bound
//! to that phase's ends put on the trace. The chain behind it names both of them, so a reader
//! sees which start and which end produced the rate.
//!
//! The phase is required and has no default. The implementing paper computes this rate for the
//! unloading, eccentric yielding and eccentric braking phases and not for the concentric one,
//! so there is no phase for a rule to fall back to, and one chosen here would put that paper's
//! citation on an interval it never took this rate across.

use crate::boundaries;
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

/// What a caller has to answer before this rule can run, with one value that answers it.
///
/// Not a default. `compute` refuses an unstated phase and the entry forbids the default that
/// would quiet it. This is what a surface offering the rule has to ask, and what a check
/// reaching the rule has to supply.
pub const REQUIRED_OPTIONS: &[(&str, &str)] = &[(
    boundaries::PHASE_PARAMETER,
    "propulsion_start_to_propulsion_end",
)];

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
    let phase = match resolved.required_enumerated(
        ID,
        boundaries::PHASE_PARAMETER,
        boundaries::PHASE_VALUES,
    ) {
        Ok(phase) => phase,
        Err(refusal) => return DerivedOutcome::declined(resolved.finish(), refusal),
    };
    let bound = resolved.finish();

    let (start, end) = match boundaries::phase_interval(context, phase) {
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
