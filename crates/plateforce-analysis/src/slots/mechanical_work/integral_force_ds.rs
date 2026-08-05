//! `work.integral_force_ds`: force integrated through the displacement it acted over.
//!
//! The registry files this and the power-time route as a naming disagreement rather than as
//! competing schools, and core keeps one integral for both. That is not a shortcut. Where a
//! measured displacement signal exists the two routes read different instruments and can
//! disagree; where none exists the displacement is the integral of the same velocity the power
//! series was formed from, so the two are one integral written twice, and shipping a second
//! would be two answers to one question.
//!
//! What separates the two entries is which inputs an implementation has, and the record says
//! which one the caller selected. `the_two_quadrature_routes_are_one_integral_here` holds the
//! equality, and holds it against a wrong weighing epoch as well, which is where the registry's
//! note about error propagation would show a difference if this build had one.

use crate::binding::ONSET_CONSTRUCT;
use crate::derived::{DerivedContext, DerivedOutcome, DerivedRule};
use crate::request::MethodChoice;
use crate::resolution::{Resolution, RuleRefusal};
use crate::response::Quantity;
use crate::slots::mechanical_power;

pub const ID: &str = "work.integral_force_ds";

pub const QUANTITIES: &[Quantity] = &[Quantity {
    key: super::KEY,
    label: "Work",
    unit: "joules",
    computed_by: Some(ID),
}];

pub const RULE: DerivedRule = compute;

fn compute(
    context: &DerivedContext,
    choice: &MethodChoice,
    _warnings: &mut Vec<String>,
) -> DerivedOutcome {
    let mut resolved = Resolution::over(&choice.parameters, &choice.options, choice.claims());

    let Some(onset) = context.onset_index() else {
        return DerivedOutcome::declined(
            resolved.finish(),
            context.unavailable(ID, &[ONSET_CONSTRUCT]),
        );
    };
    let series = mechanical_power::power_series(context, &mut resolved, ID, onset, Some(super::KEY));
    let phase = mechanical_power::phase_interval(context, &mut resolved, ID);
    let bound = resolved.finish();

    let (series, phase) = match (series, phase) {
        (Ok(series), Ok(phase)) => (series, phase),
        (Err(refusal), _) | (_, Err(refusal)) => return DerivedOutcome::declined(bound, refusal),
    };

    mechanical_power::record_entries_behind(context, super::KEY, onset);
    match plateforce_core::power::work_joules(
        &series,
        &phase,
        context.trial.sample_interval_seconds(),
    ) {
        Ok(joules) => DerivedOutcome {
            values: vec![(super::KEY, Some(joules))],
            placed: Vec::new(),
            bound,
            refusal: None,
        },
        Err(_) => DerivedOutcome::declined(
            bound,
            RuleRefusal::Refused(Box::new(plateforce_core::Refusal::span_selects_no_samples(
                ID,
                phase.first_index,
                phase.last_index,
            ))),
        ),
    }
}
