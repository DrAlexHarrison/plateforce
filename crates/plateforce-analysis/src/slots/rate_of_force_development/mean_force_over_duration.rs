//! `rfd.mean_force_over_duration.lapuente`: the mean force over a phase divided by that
//! phase's duration.
//!
//! Deprecated, and shipped so that years of numbers carrying this label stay interpretable.
//! It is not a rate with a bias, it is a different quantity: it rises with mean force and
//! falls with phase duration, so an athlete producing the same force for twice as long scores
//! half the number, where a rate of force development is a derivative. The maintainer of the
//! product that popularised the label wrote the objection into their own source at both sites
//! that compute it.
//!
//! The caller states which phase, on the same footing as the secant rule beside it, and the
//! chain names the two boundaries the number was taken between. Required with no default for
//! the same reason: the product that popularised the label computes it per phase and names no
//! primary, so a phase chosen here would sit where the caller's choice belongs.

use crate::boundaries;
use crate::derived::{DerivedContext, DerivedOutcome, DerivedRule};
use crate::request::MethodChoice;
use crate::resolution::{Resolution, RuleRefusal};
use crate::response::Quantity;

pub const ID: &str = "rfd.mean_force_over_duration.lapuente";

pub const QUANTITIES: &[Quantity] = &[Quantity {
    key: super::KEY,
    label: "Rate of force development",
    unit: "newtons_per_second",
    computed_by: Some(ID),
}];

/// What a caller has to answer before this rule can run, with one value that answers it.
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
    let mut resolved = Resolution::over(&choice.parameters, &choice.options, choice.claims());
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

    // Both boundaries are inside the phase, so the mean is taken over the closed interval
    // while the duration counts the intervals between those samples.
    let force = context.trial.force();
    let last = end.min(force.len().saturating_sub(1));
    let Some(mean_newtons) = plateforce_core::statistics::mean(&force[start..=last]) else {
        return DerivedOutcome::declined(
            bound,
            RuleRefusal::Refused(Box::new(plateforce_core::Refusal::span_selects_no_samples(
                ID, start, end,
            ))),
        );
    };

    let duration_seconds = (last - start) as f64 * context.trial.sample_interval_seconds();
    if duration_seconds <= 0.0 {
        return DerivedOutcome::declined(
            bound,
            RuleRefusal::Refused(Box::new(plateforce_core::Refusal::span_selects_no_samples(
                ID, start, last,
            ))),
        );
    }

    DerivedOutcome {
        values: vec![(super::KEY, Some(mean_newtons / duration_seconds))],
        placed: Vec::new(),
        bound,
        refusal: None,
    }
}
