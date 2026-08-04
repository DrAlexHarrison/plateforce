//! `impulse.epoch_from_onset`: force integrated from onset over a stated epoch.
//!
//! It inherits the onset bias and integrates it, so it moves less with onset than a rate at
//! the same epoch does. The registry records CV 4.3 to 8.7 percent and ICC 0.89 to 0.97
//! across epochs with posture varying, against a rate's wider spread on the same trials.
//!
//! Trapezoidal against rectangle-rule integration is not a second entry at the rates this
//! literature uses, and the core integrates trapezoidally, subtracting the convention's offset
//! inside the integral rather than after it: removed afterwards over one interval too many, a
//! whole sample of weight is left behind.

use crate::binding::ONSET_CONSTRUCT;
use crate::derived::{DerivedContext, DerivedOutcome, DerivedRule};
use crate::request::MethodChoice;
use crate::resolution::{Resolution, RuleRefusal};
use crate::response::Quantity;

pub const ID: &str = "impulse.epoch_from_onset";

/// The entry's own name for the epoch, and the value it publishes as its default.
pub const EPOCH_PARAMETER: &str = "epoch_ms";
pub const EPOCH_DEFAULT_MILLISECONDS: f64 = 200.0;

pub const QUANTITIES: &[Quantity] = &[Quantity {
    key: super::KEY,
    label: "Epoch impulse",
    unit: "newton_seconds",
    computed_by: Some(ID),
}];

pub const RULE: DerivedRule = compute;

fn compute(
    context: &DerivedContext,
    choice: &MethodChoice,
    _warnings: &mut Vec<String>,
) -> DerivedOutcome {
    let mut resolved = Resolution::over(&choice.parameters, &choice.options, choice.claims());
    let convention = super::convention(&mut resolved, ID);
    let (epoch_milliseconds, epoch_samples) = resolved.milliseconds_and_samples(
        EPOCH_PARAMETER,
        EPOCH_DEFAULT_MILLISECONDS,
        context.trial.sample_rate_hz(),
    );
    let bound = resolved.finish();

    let convention = match convention {
        Ok(convention) => convention,
        Err(refusal) => return DerivedOutcome::declined(bound, refusal),
    };

    let Some(onset) = context.onset_index() else {
        return DerivedOutcome::declined(bound, context.unavailable(ID, &[ONSET_CONSTRUCT]));
    };

    // The epoch a reader asked for and the epoch the recording holds are the same only while
    // the file runs that far, and an integral over the shorter of the two is not the number
    // the entry describes.
    let end = onset + epoch_samples;
    if end >= context.trial.len() {
        return DerivedOutcome::declined(
            bound,
            RuleRefusal::Refused(Box::new(plateforce_core::Refusal::value_not_accepted(
                ID,
                EPOCH_PARAMETER,
                epoch_milliseconds,
                vec![format!(
                    "an epoch the recording holds after onset, which is at most {:.1} ms here",
                    (context.trial.len() - 1 - onset) as f64 * 1000.0
                        / context.trial.sample_rate_hz()
                )],
            ))),
        );
    }

    super::record_entry_behind(context, super::KEY);
    let impulse_newton_seconds = context.trial.integrate_offset_newton_seconds(
        onset,
        end + 1,
        super::offset_newtons(context, convention),
    );

    DerivedOutcome {
        values: vec![(super::KEY, Some(impulse_newton_seconds))],
        placed: Vec::new(),
        bound,
        refusal: None,
    }
}
