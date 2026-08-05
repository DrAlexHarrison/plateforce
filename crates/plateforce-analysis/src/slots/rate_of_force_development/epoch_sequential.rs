//! `rfd.epoch_from_onset.sequential`: the rate over one of the consecutive non-overlapping
//! windows the entry lays end to end from onset.
//!
//! The scheme's whole claim against the onset-anchored one is in its own registry text:
//! consecutive windows localise where two force-time curves diverge, which overlapping
//! windows cannot do because every overlapping window shares the noisy first interval. That
//! claim lives in windows past the first, and the first window at width w is arithmetically
//! the overlapping scheme at epoch w.
//!
//! So the window is the caller's, stated and required. A default of the first window would
//! ship this rule as a second spelling of the entry the registry files as disagreeing with
//! it, and a rule that reported all four at once would report quantities no other rule in
//! this construct reports, which is a fork the construct declares it does not have. The
//! sequence the entry describes is reached by sweeping the index, where the rate stays one
//! key and the spread across windows is the shape the scheme exists to show.
//!
//! Sensitivity, from the entry: high to onset in every window rather than only the first.
//! Moving onset by 5 to 20 ms moved a window past the first by up to 11,855 N/s across the
//! six trials held here.

use crate::binding::ONSET_CONSTRUCT;
use crate::derived::{DerivedContext, DerivedOutcome, DerivedRule};
use crate::request::MethodChoice;
use crate::resolution::{Resolution, RuleRefusal};
use crate::response::Quantity;

pub const ID: &str = "rfd.epoch_from_onset.sequential";

/// The entry's own names. The width publishes one value and defaults to it; the index
/// publishes the four windows the source's figure lays out and defaults to nothing.
pub const WINDOW_PARAMETER: &str = "window_ms";
pub const WINDOW_DEFAULT_MILLISECONDS: f64 = 50.0;
pub const INDEX_PARAMETER: &str = "window_index";

/// The names this rule declines without, and a value that satisfies each, so a surface
/// offering it knows what to ask for and a check reaching it does not know the answer by
/// heart.
pub const REQUIRED_NUMBERS: &[(&str, f64)] = &[(INDEX_PARAMETER, 2.0)];

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
    let (_, width_samples) = resolved.milliseconds_and_samples(
        WINDOW_PARAMETER,
        WINDOW_DEFAULT_MILLISECONDS,
        context.trial.sample_rate_hz(),
    );
    let index = match resolved.required_number(ID, INDEX_PARAMETER) {
        Ok(index) => index,
        Err(refusal) => return DerivedOutcome::declined(resolved.finish(), refusal),
    };
    let bound = resolved.finish();

    let Some(onset) = context.onset_index() else {
        return DerivedOutcome::declined(bound, context.unavailable(ID, &[ONSET_CONSTRUCT]));
    };

    let windows = plateforce_core::rate::sequential_chords(
        context.trial.force(),
        onset,
        width_samples,
        context.trial.len(),
        context.trial.sample_interval_seconds(),
    );

    // Counted from one, because the source's figure numbers them from one and a reader
    // restating "the second window" means the second one. An index the recording does not
    // hold is refused against the count this trace actually supports rather than served by
    // the nearest window that exists.
    let wanted = if index >= 1.0 && index.fract() == 0.0 {
        Some(index as usize)
    } else {
        None
    };
    let Some(chord) = wanted
        .filter(|wanted| *wanted <= windows.len())
        .map(|wanted| windows[wanted - 1])
    else {
        return DerivedOutcome::declined(
            bound,
            RuleRefusal::Refused(Box::new(plateforce_core::Refusal::value_not_accepted(
                ID,
                INDEX_PARAMETER,
                index,
                vec![format!(
                    "a whole window this recording holds after onset at this width, of which \
                     there are {}",
                    windows.len()
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
