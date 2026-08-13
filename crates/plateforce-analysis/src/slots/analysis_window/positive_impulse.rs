//! `phase.window.positive_impulse.net_force_positive`: the stretch over which force stands
//! above system weight.
//!
//! The registry's text: from the rising crossing of system weight following the unweighting
//! minimum to the falling crossing during late propulsion. It holds the whole braking phase
//! and excludes the sub-system-weight segment before takeoff, which is what separates it from
//! the propulsive window a caller might otherwise reach for.
//!
//! The entry records criterion validity rather than an offset against that propulsive window:
//! on 59 athletes the positive-impulse window found an asymmetry to jump height relationship
//! in 2 of 4 sex-by-load cells and the propulsive window in 0 of 4. The claim the registry
//! makes for it is that the choice of window decides whether a relationship is detected at
//! all, not that it flips a sign, which is why the window's identity travels with any number
//! taken over it.
//!
//! Both crossings are read off the summated trace, so on dual plates each limb integrates
//! inside the shared window rather than finding its own. This build reads one trace and the
//! rule is the same one either way.

use plateforce_core::phases::positive_impulse_window;

use crate::binding::{ONSET_CONSTRUCT, TAKEOFF_CONSTRUCT};
use crate::boundaries;
use crate::derived::{DerivedContext, DerivedOutcome, DerivedRule};
use crate::request::MethodChoice;
use crate::resolution::{Resolution, RuleRefusal};
use crate::response::Quantity;

pub const ID: &str = "phase.window.positive_impulse.net_force_positive";

/// The same two keys the other window rules report, so a reader comparing windows holds the
/// key still and watches `computed_by` change.
pub const QUANTITIES: &[Quantity] = &[
    Quantity {
        key: "analysis_window_start_seconds",
        label: "Analysis window, start",
        unit: "seconds",
        computed_by: Some(ID),
        produced_by_construct: None,
    },
    Quantity {
        key: "analysis_window_end_seconds",
        label: "Analysis window, end",
        unit: "seconds",
        computed_by: Some(ID),
        produced_by_construct: None,
    },
];

pub const RULE: DerivedRule = place;

fn place(
    context: &DerivedContext,
    choice: &MethodChoice,
    _warnings: &mut Vec<String>,
) -> DerivedOutcome {
    let resolved = Resolution::over(
        &choice.parameters,
        &choice.options,
        choice.declared.of_entry(ID),
        choice.claims(),
    );
    let bound = resolved.finish();

    let (Some(onset), Some(takeoff)) = (context.onset_index(), context.takeoff_index()) else {
        let missing = boundaries::absent(context, &[ONSET_CONSTRUCT, TAKEOFF_CONSTRUCT]);
        return DerivedOutcome::declined(bound, context.unavailable(ID, &missing));
    };

    // The falling crossing is searched back from the propulsive peak, so the rising crossing
    // has to be found on the near side of it. Handing the whole interval to both searches
    // would let the rising one take a re-crossing after the peak.
    let Some(peak) = boundaries::propulsive_peak_index(context, onset, takeoff) else {
        return DerivedOutcome::declined(
            bound,
            RuleRefusal::Refused(Box::new(plateforce_core::Refusal::span_selects_no_samples(
                ID, onset, takeoff,
            ))),
        );
    };

    let system_weight_newtons = context.epoch().system_weight_newtons;
    let crossings = positive_impulse_window(
        context.trial.force(),
        system_weight_newtons,
        onset,
        peak,
        takeoff,
    );

    // Both ends or neither. A window with one end is not a window, and a reader handed one
    // end under this rule's name would be reading a boundary the rule did not place.
    let Some((start, end)) = crossings else {
        return DerivedOutcome::declined(
            bound,
            RuleRefusal::Refused(Box::new(plateforce_core::Refusal::nothing_qualified(
                ID,
                (takeoff + 1).saturating_sub(onset),
                std::collections::BTreeMap::from([
                    (
                        "interval_start_seconds".to_string(),
                        context.trial.time_at(onset),
                    ),
                    (
                        "interval_end_seconds".to_string(),
                        context.trial.time_at(takeoff),
                    ),
                    ("reference_newtons".to_string(), system_weight_newtons),
                ]),
            ))),
        );
    };

    // A crossing the core reports without the trace having crossed is the fallback one
    // shipped tool returns, and publishing it would put an instant under this rule's name
    // that no crossing produced.
    if !start.is_true_crossing || !end.is_true_crossing {
        return DerivedOutcome::declined(
            bound,
            RuleRefusal::Refused(Box::new(plateforce_core::Refusal::no_crossing(
                ID,
                "system_weight_newtons",
                system_weight_newtons,
                context.trial.time_at(takeoff),
            ))),
        );
    }

    let window_end = (end.index + 1).min(context.trial.len());
    DerivedOutcome {
        values: vec![
            (
                "analysis_window_start_seconds",
                Some(context.trial.time_at(start.index)),
            ),
            (
                "analysis_window_end_seconds",
                Some(context.trial.time_at(window_end.saturating_sub(1))),
            ),
        ],
        placed: vec![(super::START, start.index), (super::END, window_end)],
        bound,
        refusal: None,
    }
}
