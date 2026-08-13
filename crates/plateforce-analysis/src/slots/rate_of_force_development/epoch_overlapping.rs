//! `rfd.epoch_from_onset.overlapping`: the mean rate over a stated interval from onset.
//!
//! The whole of the onset bias arrives here and decays across the epoch. The registry
//! records +266 N at 50 ms, +289 N at 90 ms and +214 N at 150 ms against a manually placed
//! onset under a 5 SD rule, which is a large effect at the short epochs and trivial by 250 ms.
//!
//! The chord is taken from the force at onset rather than from zero, so a gross convention
//! does not put a whole system weight into the numerator: the entry states that the force at
//! the epoch and the rate over it are one quantity in two units under a net convention and
//! two quantities under a gross one, and this reports the rate.

use crate::binding::ONSET_CONSTRUCT;
use crate::derived::{DerivedContext, DerivedOutcome, DerivedRule};
use crate::request::MethodChoice;
use crate::resolution::{Resolution, RuleRefusal};
use crate::response::Quantity;

pub const ID: &str = "rfd.epoch_from_onset.overlapping";

/// The entry's own name for the epoch, and the value it publishes as its default.
pub const EPOCH_PARAMETER: &str = "epoch_ms";
pub const EPOCH_DEFAULT_MILLISECONDS: f64 = 200.0;

pub const QUANTITIES: &[Quantity] = &[Quantity {
    key: super::KEY,
    label: "Rate of force development",
    unit: "newtons_per_second",
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
        // The epoch is the number a reader restates, so the refusal names it and says how
        // much recording there is after onset rather than reporting a shorter interval under
        // the length that was asked for.
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
        values: vec![(super::KEY, Some(chord.rate_newtons_per_second()))],
        placed: Vec::new(),
        bound,
        refusal: None,
    }
}
