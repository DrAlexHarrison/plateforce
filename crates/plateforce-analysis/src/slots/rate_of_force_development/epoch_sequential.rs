//! `rfd.epoch_from_onset.sequential`: consecutive windows of one width laid end to end from
//! onset, none overlapping.
//!
//! Four windows, because the figure this rule is transcribed from lays four of them end to
//! end to 200 ms, and because the scheme's argument is about where two force-time curves
//! diverge, which needs more than the first. A window the recording does not hold reports no
//! value rather than a rate taken over a shorter span.
//!
//! The first window is reported under the shared rate key and the rest under their own. At a
//! 50 ms width the first window is the same interval the overlapping scheme measures at a
//! 50 ms epoch, so the two schemes are directly comparable where they agree and separate
//! after it, which is the information argument the entry is defended on.
//!
//! Every window inherits onset, not only the first: moving onset by 5 to 20 ms moved a
//! window past the first by up to 11,855 N/s across the six trials held here.

use crate::binding::ONSET_CONSTRUCT;
use crate::derived::{DerivedContext, DerivedOutcome, DerivedRule};
use crate::request::MethodChoice;
use crate::resolution::{Resolution, RuleRefusal};
use crate::response::Quantity;
use crate::slots::analysis_window;

pub const ID: &str = "rfd.epoch_from_onset.sequential";

/// The entry's own name for the width, and the one value it publishes.
pub const WINDOW_PARAMETER: &str = "window_ms";
pub const WINDOW_DEFAULT_MILLISECONDS: f64 = 50.0;

/// The windows past the first, each under its own key because they are different intervals
/// of the trace rather than two answers to one question.
pub const SECOND_KEY: &str = "rate_of_force_development_window_2_newtons_per_second";
pub const THIRD_KEY: &str = "rate_of_force_development_window_3_newtons_per_second";
pub const FOURTH_KEY: &str = "rate_of_force_development_window_4_newtons_per_second";

pub const QUANTITIES: &[Quantity] = &[
    Quantity {
        key: super::KEY,
        label: "Rate of force development",
        unit: "newtons_per_second",
        computed_by: Some(ID),
    },
    Quantity {
        key: SECOND_KEY,
        label: "Rate of force development, second window",
        unit: "newtons_per_second",
        computed_by: Some(ID),
    },
    Quantity {
        key: THIRD_KEY,
        label: "Rate of force development, third window",
        unit: "newtons_per_second",
        computed_by: Some(ID),
    },
    Quantity {
        key: FOURTH_KEY,
        label: "Rate of force development, fourth window",
        unit: "newtons_per_second",
        computed_by: Some(ID),
    },
];

/// The keys in trace order, so the windows and the keys cannot fall out of step.
const KEYS: [&str; 4] = [super::KEY, SECOND_KEY, THIRD_KEY, FOURTH_KEY];

pub const RULE: DerivedRule = compute;

fn compute(
    context: &DerivedContext,
    choice: &MethodChoice,
    _warnings: &mut Vec<String>,
) -> DerivedOutcome {
    let mut resolved = Resolution::over(&choice.parameters, &choice.options, choice.claims());
    let (window_milliseconds, window_samples) = resolved.milliseconds_and_samples(
        WINDOW_PARAMETER,
        WINDOW_DEFAULT_MILLISECONDS,
        context.trial.sample_rate_hz(),
    );
    let bound = resolved.finish();

    let onset = context.onset_index();
    let window = analysis_window::span(context);
    let (Some(onset), Some((_, window_end))) = (onset, window) else {
        let mut missing = Vec::new();
        if onset.is_none() {
            missing.push(ONSET_CONSTRUCT);
        }
        if window.is_none() {
            missing.push(analysis_window::CONSTRUCT);
        }
        return DerivedOutcome::declined(bound, context.unavailable(ID, &missing));
    };

    let chords = plateforce_core::rate::sequential_chords(
        context.trial.force(),
        onset,
        window_samples,
        window_end,
        context.trial.sample_interval_seconds(),
    );
    if chords.is_empty() {
        return DerivedOutcome::declined(
            bound,
            RuleRefusal::Refused(Box::new(plateforce_core::Refusal::value_not_accepted(
                ID,
                WINDOW_PARAMETER,
                window_milliseconds,
                vec![format!(
                    "a width the analysis window holds after onset, which is at most {:.1} ms here",
                    window_end.saturating_sub(onset) as f64 * 1000.0
                        / context.trial.sample_rate_hz()
                )],
            ))),
        );
    }

    // Over the keys rather than over the chords, so a recording holding fewer than four
    // whole windows reports the rest as quantities with no value. A reader can see a blank
    // cell; nobody can see a key that never arrived.
    DerivedOutcome {
        values: KEYS
            .iter()
            .enumerate()
            .map(|(position, key)| {
                (
                    *key,
                    chords
                        .get(position)
                        .map(plateforce_core::rate::Chord::rate_newtons_per_second),
                )
            })
            .collect(),
        placed: Vec::new(),
        bound,
        refusal: None,
    }
}
