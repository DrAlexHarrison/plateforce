//! `rfd.at_fraction_of_peak_force`: the centred derivative where raw force first reaches a
//! stated fraction of peak force.
//!
//! The one rate variant nearly independent of onset placement, and it pays for that with a
//! dependence on peak force: on a noisy peak, peak force is biased high, which moves the
//! evaluation point later and, on a decelerating rise, reports a lower rate. So the number
//! moves with filtering where the epoch rules move with onset.
//!
//! Both the crossing and the derivative are read between samples. At 1200 Hz one sample
//! interval is 0.83 ms, and reading the derivative at the sample after the crossing rather
//! than at the crossing moves the answer by whatever the curvature of the rise puts there.

use crate::derived::{DerivedContext, DerivedOutcome, DerivedRule};
use crate::request::MethodChoice;
use crate::resolution::{Resolution, RuleRefusal};
use crate::response::Quantity;
use crate::slots::analysis_window;

pub const ID: &str = "rfd.at_fraction_of_peak_force";

/// The entry's own name for the fraction, and the value it publishes as its default.
pub const FRACTION_PARAMETER: &str = "fraction_pct";
pub const FRACTION_DEFAULT_PERCENT: f64 = 50.0;

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
    let mut resolved = Resolution::over(
        &choice.parameters,
        &choice.options,
        choice.declared.of_entry(ID),
        choice.claims(),
    );
    let fraction_percent = resolved.number(FRACTION_PARAMETER, FRACTION_DEFAULT_PERCENT);
    let bound = resolved.finish();

    let Some((start, end)) = analysis_window::span(context) else {
        return DerivedOutcome::declined(
            bound,
            context.unavailable(ID, &[analysis_window::CONSTRUCT]),
        );
    };

    let force = context.trial.force();
    let Ok(peak_newtons) = plateforce_core::peak::maximum_over(force, start, end) else {
        return DerivedOutcome::declined(
            bound,
            RuleRefusal::Refused(Box::new(plateforce_core::Refusal::span_selects_no_samples(
                ID, start, end,
            ))),
        );
    };

    let level_newtons = peak_newtons * fraction_percent / 100.0;
    let crossing = plateforce_core::rate::first_crossing_at_or_above(
        force,
        level_newtons,
        start,
        end.saturating_sub(1),
    );
    // A fraction above 100 asks for a force above the peak the window holds, which is the
    // stated number rather than the recording.
    let Some(crossing) = crossing else {
        return DerivedOutcome::declined(
            bound,
            RuleRefusal::Refused(Box::new(plateforce_core::Refusal::value_not_accepted(
                ID,
                FRACTION_PARAMETER,
                fraction_percent,
                vec![format!(
                    "a fraction of the {peak_newtons:.1} N peak that force reaches inside the analysis window"
                )],
            ))),
        );
    };

    let Some(rate) = plateforce_core::rate::centred_derivative_at(
        force,
        crossing.position,
        context.trial.sample_interval_seconds(),
    ) else {
        // The crossing landed at an edge of the recording, where no sample sits either side
        // of it. A one-sided difference reported here would be a different quantity under
        // this rule's name.
        return DerivedOutcome::declined(
            bound,
            RuleRefusal::Refused(Box::new(plateforce_core::Refusal::span_selects_no_samples(
                ID,
                crossing.sample_index.saturating_sub(1),
                (crossing.sample_index + 2).min(force.len()),
            ))),
        );
    };

    DerivedOutcome {
        values: vec![(super::KEY, Some(rate))],
        placed: Vec::new(),
        bound,
        refusal: None,
    }
}
