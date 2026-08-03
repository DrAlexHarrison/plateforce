//! `phase.anchor.time_epochs.schmidtbleicher`: fixed intervals from onset, not kinematic
//! events.
//!
//! A fork in what kind of object a phase is rather than a fork within one quantity. Time
//! anchoring is strategy-independent and therefore comparable across people who move
//! differently, which is a feature for cross-subject comparison and a bug for individual
//! diagnosis, and the reverse holds for event anchoring.
//!
//! The epoch is measured from contraction onset, so it rests on the bound onset rule and moves
//! with it, and it is the only boundary here that does not read the trace after onset at all.

use crate::binding::ONSET_CONSTRUCT;
use crate::boundaries;
use crate::derived::{DerivedContext, DerivedOutcome, DerivedRule};
use crate::request::MethodChoice;
use crate::resolution::Resolution;
use crate::response::Quantity;

pub const ID: &str = "phase.anchor.time_epochs.schmidtbleicher";

/// The entry publishes 30, 50, 100, 200 and 250 ms and names 200 as its default.
const PUBLISHED_EPOCH_MILLISECONDS: f64 = 200.0;

pub const KEY: &str = "time_epoch_end_seconds";
pub const PLACED: &str = "time_epoch_end";

pub const QUANTITIES: &[Quantity] = &[Quantity {
    key: KEY,
    label: "End of the epoch measured from onset",
    unit: "seconds",
    computed_by: Some(ID),
}];

pub const RULE: DerivedRule = place;

fn place(
    context: &DerivedContext,
    choice: &MethodChoice,
    _warnings: &mut Vec<String>,
) -> DerivedOutcome {
    let mut resolved = Resolution::over(&choice.parameters, &choice.options, choice.claims());
    let epoch_samples = resolved.milliseconds_as_samples(
        "epoch_ms",
        PUBLISHED_EPOCH_MILLISECONDS,
        context.trial.sample_rate_hz(),
    );
    let bound = resolved.finish();

    let Some(onset) = context.onset_index else {
        let missing = boundaries::absent(context, &[ONSET_CONSTRUCT]);
        return DerivedOutcome::declined(bound, context.unavailable(ID, &missing));
    };

    // An epoch running off the end of the recording is not an epoch. Reported as no value
    // rather than clipped, because a clipped epoch is a shorter interval under the length the
    // caller asked for.
    let end = onset + epoch_samples;
    let index = (end < context.trial.len()).then_some(end);
    boundaries::placed_outcome(context, KEY, PLACED, index, bound)
}
