//! `rpd.peak_to_peak_anchored.amti`: the line from the negative peak power to the positive
//! peak power that follows it, one value for the jump.
//!
//! Legacy compatibility. It produced reference values in the validation corpus on a superseded
//! product, and it is retained so those numbers stay interpretable.
//!
//! Deliberately insensitive to the phase model, which makes it unusually robust to onset
//! placement: a rare property in this registry. Both ends of the line are sample extremes of a
//! noisy series, and such an extreme drifts outward as the sampling rate rises, so this rule
//! carries that drift at both ends where a rule reading one extreme carries it at one. Nothing
//! in the sweep quantifies it, which makes values from recordings at different sampling rates
//! not directly comparable under this rule.
//!
//! Insensitive to the phase model is not insensitive to the window. The search runs over the
//! analysis window, because a recording that continues past takeoff puts the trough at the
//! landing, where force is large and centre-of-mass velocity is negative, and the line would
//! then be drawn between two instants of the landing.

use crate::binding::ONSET_CONSTRUCT;
use crate::derived::{DerivedContext, DerivedOutcome, DerivedRule};
use crate::request::MethodChoice;
use crate::resolution::{Resolution, RuleRefusal};
use crate::response::Quantity;
use crate::slots::analysis_window;

pub const ID: &str = "rpd.peak_to_peak_anchored.amti";

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
    let series = super::power_series(context, &mut resolved, ID, onset, Some(super::KEY));
    let bound = resolved.finish();

    let series = match series {
        Ok(series) => series,
        Err(refusal) => return DerivedOutcome::declined(bound, refusal),
    };

    let Some((start, end)) = analysis_window::span(context) else {
        return DerivedOutcome::declined(
            bound,
            context.unavailable(ID, &[analysis_window::CONSTRUCT]),
        );
    };

    super::record_entries_behind(context, onset);
    match plateforce_core::power::rate_of_power_development_peak_to_peak(
        &series,
        start,
        end,
        context.trial.sample_interval_seconds(),
    ) {
        Ok(rate) => DerivedOutcome {
            values: vec![(super::KEY, Some(rate.watts_per_second))],
            placed: Vec::new(),
            bound,
            refusal: None,
        },
        Err(_) if end.saturating_sub(start) < 2 => DerivedOutcome::declined(
            bound,
            RuleRefusal::Refused(Box::new(plateforce_core::Refusal::span_selects_no_samples(
                ID, start, end,
            ))),
        ),
        // The window holds samples and its lowest power is the last of them, so no peak
        // follows the trough. On a countermovement jump under the net force term that is
        // where the trough sits: force has fallen away from system weight while velocity is
        // at its largest, and the product of the two is the most negative power in the jump.
        Err(_) => DerivedOutcome::declined(
            bound,
            RuleRefusal::Refused(Box::new(plateforce_core::Refusal::nothing_qualified(
                ID,
                end - start,
                std::collections::BTreeMap::from([(
                    "search_bound_seconds".to_string(),
                    context.trial.time_at(end.saturating_sub(1)),
                )]),
            ))),
        ),
    }
}
