//! `rpd.phase_anchored`: the line from the start of a declared phase to the instant of peak
//! power inside it.
//!
//! The phase is the one the propulsion rules declared, so this rate inherits the phase model
//! and moves with it. The chain names both boundaries, which is what lets a reader see why two
//! results under this entry differ.

use crate::binding::ONSET_CONSTRUCT;
use crate::derived::{DerivedContext, DerivedOutcome, DerivedRule};
use crate::request::MethodChoice;
use crate::resolution::{Resolution, RuleRefusal};
use crate::response::Quantity;

pub const ID: &str = "rpd.phase_anchored";

pub const QUANTITIES: &[Quantity] = &[Quantity {
    key: super::KEY,
    label: "Rate of power development",
    unit: "watts_per_second",
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
    let series = super::power_series(context, &mut resolved, ID, onset, Some(super::KEY));
    let bound = resolved.finish();

    let series = match series {
        Ok(series) => series,
        Err(refusal) => return DerivedOutcome::declined(bound, refusal),
    };

    let (start, end) = match crate::slots::rate_of_force_development::propulsion_interval(context) {
        Ok(interval) => interval,
        Err(missing) => return DerivedOutcome::declined(bound, context.unavailable(ID, &missing)),
    };

    super::record_entries_behind(context, onset);
    let phase = plateforce_core::power::DeclaredPhase {
        first_index: start,
        last_index: end,
        method_id: ID.to_string(),
    };
    match plateforce_core::power::rate_of_power_development_phase_anchored(
        &series,
        &phase,
        context.trial.sample_interval_seconds(),
    ) {
        Ok(rate) => DerivedOutcome {
            values: vec![(super::KEY, Some(rate.watts_per_second))],
            placed: Vec::new(),
            bound,
            refusal: None,
        },
        // A phase running off the trace, or one of a single sample, is the boundaries rather
        // than this rule, and the interval is what a reader moves to fix it.
        Err(_) if end.saturating_sub(start) < 2 || end >= context.trial.len() => {
            DerivedOutcome::declined(
                bound,
                RuleRefusal::Refused(Box::new(plateforce_core::Refusal::span_selects_no_samples(
                    ID, start, end,
                ))),
            )
        }
        // Peak power at the first sample of the phase leaves no line to draw. The phase was
        // read and every sample in it was compared, so it is the recording answering rather
        // than an interval nobody could search.
        Err(_) => DerivedOutcome::declined(
            bound,
            RuleRefusal::Refused(Box::new(plateforce_core::Refusal::nothing_qualified(
                ID,
                end - start,
                std::collections::BTreeMap::from([(
                    "search_bound_seconds".to_string(),
                    context.trial.time_at(end),
                )]),
            ))),
        ),
    }
}
