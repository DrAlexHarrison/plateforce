//! `impulse.to_fraction_of_peak_force`: force integrated from onset until it first reaches a
//! stated fraction of peak force.
//!
//! The same time-against-force anchoring fork as the rate pair: it trades the epoch rule's
//! sensitivity to onset for a sensitivity to peak force, so on a noisy peak the interval runs
//! longer and the impulse comes out larger.
//!
//! The source this rule is transcribed from loops without an upper bound and runs off the end
//! of its array when the threshold is never met. Here the search is bounded by the analysis
//! window and a threshold never reached is a refusal naming the fraction.

use crate::binding::ONSET_CONSTRUCT;
use crate::derived::{DerivedContext, DerivedOutcome, DerivedRule};
use crate::request::MethodChoice;
use crate::resolution::{Resolution, RuleRefusal};
use crate::response::Quantity;
use crate::slots::analysis_window;

pub const ID: &str = "impulse.to_fraction_of_peak_force";

/// The entry's own name for the fraction, and the value it publishes as its default.
pub const FRACTION_PARAMETER: &str = "fraction_pct";
pub const FRACTION_DEFAULT_PERCENT: f64 = 50.0;

pub const QUANTITIES: &[Quantity] = &[Quantity {
    key: super::KEY,
    label: "Epoch impulse",
    unit: "newton_seconds",
    computed_by: Some(ID),
    produced_by_construct: None,
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
    let convention = super::convention(&mut resolved, ID);
    let fraction_percent = resolved.number(FRACTION_PARAMETER, FRACTION_DEFAULT_PERCENT);
    let bound = resolved.finish();

    let convention = match convention {
        Ok(convention) => convention,
        Err(refusal) => return DerivedOutcome::declined(bound, refusal),
    };

    let onset = context.onset_index();
    let window = analysis_window::span(context);
    let (Some(onset), Some((start, end))) = (onset, window) else {
        let mut missing = Vec::new();
        if onset.is_none() {
            missing.push(ONSET_CONSTRUCT);
        }
        if window.is_none() {
            missing.push(analysis_window::CONSTRUCT);
        }
        return DerivedOutcome::declined(bound, context.unavailable(ID, &missing));
    };

    let force = context.trial.force();
    let peak_newtons = match plateforce_core::peak::maximum_over(force, start, end) {
        Ok(peak) => peak,
        Err(plateforce_core::peak::PeakError::SamplesCarryNoNumber(missing)) => {
            return DerivedOutcome::declined(
                bound,
                RuleRefusal::Refused(Box::new(missing.refusal(ID))),
            )
        }
        Err(_) => {
            return DerivedOutcome::declined(
                bound,
                RuleRefusal::Refused(Box::new(plateforce_core::Refusal::span_selects_no_samples(
                    ID, start, end,
                ))),
            )
        }
    };

    let level_newtons = peak_newtons * fraction_percent / 100.0;
    let crossing = match plateforce_core::rate::first_crossing_at_or_above(
        force,
        level_newtons,
        onset,
        end.saturating_sub(1),
    ) {
        Ok(crossing) => crossing,
        Err(missing) => {
            return DerivedOutcome::declined(
                bound,
                RuleRefusal::Refused(Box::new(missing.refusal(ID))),
            )
        }
    };
    let Some(crossing) = crossing else {
        return DerivedOutcome::declined(
            bound,
            RuleRefusal::Refused(Box::new(plateforce_core::Refusal::value_not_accepted(
                ID,
                FRACTION_PARAMETER,
                fraction_percent,
                vec![format!(
                    "a fraction of the {peak_newtons:.1} N peak that force reaches between onset and the end of the analysis window"
                )],
            ))),
        );
    };

    super::record_entry_behind(context, super::KEY);
    // Integrated to the first sample at or above the level rather than to the interpolated
    // crossing between two samples: the integral adds up samples, and a partial trapezoid at
    // the far end would be an interval the entry does not describe.
    let impulse_newton_seconds = context.trial.integrate_offset_newton_seconds(
        onset,
        crossing.sample_index + 1,
        super::offset_newtons(context, convention),
    );

    DerivedOutcome {
        values: vec![(super::KEY, Some(impulse_newton_seconds))],
        placed: Vec::new(),
        bound,
        refusal: None,
    }
}
