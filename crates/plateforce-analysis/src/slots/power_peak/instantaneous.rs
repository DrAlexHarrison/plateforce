//! `power.peak.instantaneous`: the largest value the power series reaches inside a declared
//! phase.
//!
//! The phase is the load-bearing choice and the registry says it is usually omitted: a peak
//! over the push, over the whole movement and over the whole recording including the landing
//! are three different numbers from one trace. So the phase is required, its four values name
//! boundary pairs other constructs placed, and the chain behind the number carries the rules
//! that placed them.
//!
//! Two market-leading products differ by 62 percent on drop-jump peak power, published and
//! unresolved, with the source's own comment that the reasons are unclear. The peak is
//! otherwise the reliable one in this family, ICC 0.98 on 112 athletes.

use crate::binding::ONSET_CONSTRUCT;
use crate::derived::{DerivedContext, DerivedOutcome, DerivedRule};
use crate::request::MethodChoice;
use crate::resolution::{Resolution, RuleRefusal};
use crate::response::Quantity;
use crate::slots::mechanical_power;

pub const ID: &str = "power.peak.instantaneous";

pub const QUANTITIES: &[Quantity] = &[Quantity {
    key: super::KEY,
    label: "Peak power",
    unit: "watts",
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
    // Every name is consulted before any of them is judged, so a request stating two of the
    // three does not report either as a name this rule never read.
    let series = mechanical_power::power_series(context, &mut resolved, ID, onset, Some(super::KEY));
    let phase = mechanical_power::phase_interval(context, &mut resolved, ID);
    let bound = resolved.finish();

    let (series, phase) = match (series, phase) {
        (Ok(series), Ok(phase)) => (series, phase),
        (Err(refusal), _) | (_, Err(refusal)) => return DerivedOutcome::declined(bound, refusal),
    };

    mechanical_power::record_entries_behind(context, super::KEY, onset);
    match plateforce_core::power::peak_power_watts(&series, &phase) {
        Ok(peak) => DerivedOutcome {
            values: vec![(super::KEY, Some(peak.watts))],
            placed: Vec::new(),
            bound,
            refusal: None,
        },
        // A phase running off the trace, or one of a single sample, is the boundaries rather
        // than this rule, and the interval is what a reader moves to fix it.
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
