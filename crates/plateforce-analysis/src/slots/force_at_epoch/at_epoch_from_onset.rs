//! `force.at_epoch_from_onset`: the force at a stated elapsed time from onset.
//!
//! The same chord the rate rules take, read at its far end rather than across its span, so
//! the two numbers cannot disagree about which samples they were taken between. The refusal
//! names the epoch and says how much recording there is after onset, for the same reason the
//! rate rule's does: the epoch is the number a reader restates, and a shorter interval
//! reported under the length that was asked for would be a different quantity wearing its
//! name.

use crate::binding::ONSET_CONSTRUCT;
use crate::derived::{DerivedContext, DerivedOutcome, DerivedRule};
use crate::request::MethodChoice;
use crate::resolution::{Resolution, RuleRefusal};
use crate::response::Quantity;

pub const ID: &str = "force.at_epoch_from_onset";

/// The entry's own name for the epoch, and the value it publishes as its default. The same
/// two the rate over the interval publishes, so a reader setting one sets the other.
pub const EPOCH_PARAMETER: &str = "epoch_ms";
pub const EPOCH_DEFAULT_MILLISECONDS: f64 = 200.0;

pub const QUANTITIES: &[Quantity] = &[Quantity {
    key: super::KEY,
    label: "Force reached at the stated time",
    unit: "newtons",
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
    let (epoch_milliseconds, epoch_samples) = resolved.milliseconds_and_samples(
        EPOCH_PARAMETER,
        EPOCH_DEFAULT_MILLISECONDS,
        context.trial.sample_rate_hz(),
    );
    let bound = resolved.finish();

    let Some(onset) = context.onset_index() else {
        return DerivedOutcome::declined(bound, context.unavailable(ID, &[ONSET_CONSTRUCT]));
    };

    let Some(chord) = plateforce_core::rate::chord(
        context.trial.force(),
        onset,
        onset + epoch_samples,
        context.trial.sample_interval_seconds(),
    ) else {
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
    };

    DerivedOutcome {
        values: vec![(
            super::KEY,
            Some(chord.force_at_end_newtons(context.trial.force())),
        )],
        placed: Vec::new(),
        bound,
        refusal: None,
    }
}
