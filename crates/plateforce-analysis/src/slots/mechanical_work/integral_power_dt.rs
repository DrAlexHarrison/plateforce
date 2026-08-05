//! `work.integral_power_dt`: the area under the power-time curve across a declared phase.
//!
//! One integration deep. The velocity the power series is built on comes from integrating net
//! force once, so an error in the weighing epoch reaches this number linearly, against the
//! quadratic path it takes through the displacement route beside it.

use crate::binding::ONSET_CONSTRUCT;
use crate::derived::{DerivedContext, DerivedOutcome, DerivedRule};
use crate::request::MethodChoice;
use crate::resolution::{Resolution, RuleRefusal};
use crate::response::Quantity;
use crate::slots::mechanical_power;

pub const ID: &str = "work.integral_power_dt";

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
    let mut resolved = Resolution::over(
        &choice.parameters,
        &choice.options,
        choice.declared.of_entry(ID),
        choice.claims(),
    );

    let Some(onset) = context.onset_index() else {
        return DerivedOutcome::declined(
            resolved.finish(),
            context.unavailable(ID, &[ONSET_CONSTRUCT]),
        );
    };
    let series =
        mechanical_power::power_series(context, &mut resolved, ID, onset, Some(super::KEY));
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
