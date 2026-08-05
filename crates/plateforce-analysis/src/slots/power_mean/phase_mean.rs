//! `power.mean.phase`: the power series averaged across a declared phase.
//!
//! The entry states that the sample average and the work over the duration are one quantity in
//! discrete data up to quadrature error, so this takes the second and
//! `the_mean_matches_the_sample_average_to_quadrature_error` measures the gap on a real trace
//! rather than asserting it.
//!
//! Two commercial packages differ by 288 percent on mean braking power with an ordinary least
//! products slope of -2.53, which is partly a genuine interval mismatch and partly a sign
//! convention nobody managed. Both of those are named choices here.

use crate::binding::ONSET_CONSTRUCT;
use crate::derived::{DerivedContext, DerivedOutcome, DerivedRule};
use crate::request::MethodChoice;
use crate::resolution::{Resolution, RuleRefusal};
use crate::response::Quantity;
use crate::slots::mechanical_power;

pub const ID: &str = "power.mean.phase";

pub const QUANTITIES: &[Quantity] = &[Quantity {
    key: super::KEY,
    label: "Mean power",
    unit: "watts",
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
    let series = mechanical_power::power_series(context, &mut resolved, ID, onset, Some(super::KEY));
    let phase = mechanical_power::phase_interval(context, &mut resolved, ID);
    let bound = resolved.finish();

    let (series, phase) = match (series, phase) {
        (Ok(series), Ok(phase)) => (series, phase),
        (Err(refusal), _) | (_, Err(refusal)) => return DerivedOutcome::declined(bound, refusal),
    };

    mechanical_power::record_entries_behind(context, super::KEY, onset);
    match plateforce_core::power::mean_power_watts(
        &series,
        &phase,
        context.trial.sample_interval_seconds(),
    ) {
        Ok(mean) => DerivedOutcome {
            values: vec![(super::KEY, Some(mean.watts))],
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
