//! What a phase boundary rule reads before it can place anything: which landmarks it is
//! missing, the centre-of-mass velocity curve, and the bound a crossing search runs to.
//!
//! `integration.toml` states four choices on constructs of their own: the quadrature the
//! net-force integral is evaluated by, the direction it runs, where it starts, and where the
//! constant is pinned. This build runs no rule for any of the four, so a rule that reads a
//! velocity states all four here rather than each rule stating its own, and the four ids are
//! written into that rule's bound values under `ParameterSource::Assumed`. A boundary that
//! moved with an integration setting no fingerprint carried would be the defect this registry
//! documents, wearing our own badge.
//!
//! The forward spec is the one `takeoff_velocity_meters_per_second` reads its impulse-momentum
//! identity off, so a velocity landmark and the takeoff velocity beside it come from one curve.

use std::collections::BTreeMap;

use plateforce_core::phases::{BoundedCrossing, PhaseModelOutcome};

use crate::derived::{DerivedContext, DerivedOutcome};
use crate::resolution::{BoundValues, RuleRefusal};

/// A rule that searched for an instant, as the outcome carrying the time it reports and the
/// sample a later rule reads it by.
///
/// `None` is the rule running and this recording carrying no such instant, which is a
/// different report from the rule declining, so it reaches a reader as a quantity with no
/// value rather than as a refusal.
pub(crate) fn placed_outcome(
    context: &DerivedContext,
    key: &'static str,
    name: &'static str,
    index: Option<usize>,
    bound: BoundValues,
) -> DerivedOutcome {
    match index {
        Some(index) => DerivedOutcome {
            values: vec![(key, Some(context.trial.time_at(index)))],
            placed: vec![(name, index)],
            bound,
            refusal: None,
        },
        None => DerivedOutcome {
            values: vec![(key, None)],
            placed: Vec::new(),
            bound,
            refusal: None,
        },
    }
}

/// The same, for a search over a trace that may carry no such crossing at all.
///
/// A recording where force steps to flight without descending through system weight has no
/// falling crossing, and that is a fact about the recording rather than an empty cell. It
/// reaches a reader as a refusal under the code for a search that found nothing, so it is as
/// visible as a number.
pub(crate) fn crossing_or_refusal(
    context: &DerivedContext,
    method_id: &str,
    key: &'static str,
    name: &'static str,
    crossing: Option<BoundedCrossing>,
    bound: BoundValues,
) -> DerivedOutcome {
    match crossing {
        Some(crossing) if crossing.is_true_crossing => {
            placed_outcome(context, key, name, Some(crossing.index), bound)
        }
        Some(fallback) => declined_at_a_fallback(context, method_id, bound, fallback),
        None => DerivedOutcome::declined(
            bound,
            RuleRefusal::Refused(Box::new(plateforce_core::Refusal::nothing_qualified(
                method_id,
                0,
                BTreeMap::new(),
            ))),
        ),
    }
}

/// The same, for a search whose answer carries whether the signal really crossed.
///
/// The core returns a fallback index when the signal never returns through the threshold,
/// which reproduces one shipped tool and is not the same quantity. Reporting it would publish
/// an instant under a crossing rule's name that no crossing produced.
pub(crate) fn crossing_outcome(
    context: &DerivedContext,
    method_id: &str,
    key: &'static str,
    name: &'static str,
    crossing: Option<BoundedCrossing>,
    bound: BoundValues,
) -> DerivedOutcome {
    match crossing {
        Some(crossing) if crossing.is_true_crossing => {
            placed_outcome(context, key, name, Some(crossing.index), bound)
        }
        Some(fallback) => declined_at_a_fallback(context, method_id, bound, fallback),
        None => placed_outcome(context, key, name, None, bound),
    }
}

/// A search that returned an instant without meeting what the rule names, refused with the
/// two instants a reader needs to measure the interval it collapsed to.
///
/// Both are in seconds, because every other number a reader compares them against is, and the
/// pair is the evidence: an interval of one sample is a boundary the recording did not carry.
fn declined_at_a_fallback(
    context: &DerivedContext,
    method_id: &str,
    bound: BoundValues,
    fallback: BoundedCrossing,
) -> DerivedOutcome {
    DerivedOutcome::declined(
        bound,
        RuleRefusal::Refused(Box::new(plateforce_core::Refusal::nothing_qualified(
            method_id,
            1,
            BTreeMap::from([
                (
                    "search_anchor_seconds".to_string(),
                    context.trial.time_at(fallback.anchor_index),
                ),
                (
                    "returned_seconds".to_string(),
                    context.trial.time_at(fallback.index),
                ),
            ]),
        ))),
    )
}

/// A phase model's five or two boundaries, or the refusal that says which of them the
/// recording did not carry.
///
/// A model declines whole rather than publishing the boundaries below the one it could not
/// place, because the intervals between them are what the model asserts and an interval with
/// an unmet end is not one of them.
pub(crate) fn model_outcome(
    context: &DerivedContext,
    method_id: &str,
    keys: &[&'static str],
    outcome: PhaseModelOutcome,
    bound: BoundValues,
) -> DerivedOutcome {
    match outcome {
        PhaseModelOutcome::Placed(model) => DerivedOutcome {
            values: keys
                .iter()
                .zip(&model.indices)
                .map(|(key, index)| (*key, Some(context.trial.time_at(*index))))
                .collect(),
            placed: Vec::new(),
            bound,
            refusal: None,
        },
        PhaseModelOutcome::BoundaryNotCrossed {
            boundary_position,
            anchor_index,
            returned_index,
        } => DerivedOutcome::declined(
            bound,
            RuleRefusal::Refused(Box::new(plateforce_core::Refusal::nothing_qualified(
                method_id,
                1,
                BTreeMap::from([
                    ("boundary_position".to_string(), boundary_position as f64),
                    (
                        "search_anchor_seconds".to_string(),
                        context.trial.time_at(anchor_index),
                    ),
                    (
                        "returned_seconds".to_string(),
                        context.trial.time_at(returned_index),
                    ),
                ]),
            ))),
        ),
        PhaseModelOutcome::NothingToPlace => DerivedOutcome {
            values: keys.iter().map(|key| (*key, None)).collect(),
            placed: Vec::new(),
            bound,
            refusal: None,
        },
    }
}

/// The construct a caller settles by stating where the athlete landed.
pub(crate) const LANDING_CONSTRUCT: &str = "landing";

/// Which of the landmarks a rule named it did not get, in the order it named them.
///
/// Naming all of a rule's inputs when one is absent sends a reader to repair a rule that
/// answered, so the list carries only what is actually missing.
pub(crate) fn absent<'a>(context: &DerivedContext, needs: &[&'a str]) -> Vec<&'a str> {
    needs
        .iter()
        .copied()
        .filter(|construct| match *construct {
            crate::binding::ONSET_CONSTRUCT => context.onset_index().is_none(),
            crate::binding::TAKEOFF_CONSTRUCT => context.takeoff_index().is_none(),
            LANDING_CONSTRUCT => context.touchdown_index().is_none(),
            _ => false,
        })
        .collect()
}

/// The sample of greatest force between two landmarks, bounding a crossing search short of
/// the collapse toward zero that precedes takeoff.
///
/// Not a choice a caller makes, and the reason is structural rather than lucky. A search this
/// bounds returns the same instant for every bound from the crossing to the sample before
/// force falls back through the reference, and the peak is strictly inside that band, because
/// force has to rise through the reference to reach a maximum above it and to fall back
/// through the reference afterwards. Bounded at takeoff instead, the search reaches the
/// collapse toward zero, where force is under the reference again, and lands there. Shown by
/// `plateforce_core::phases::a_rising_crossing_is_fixed_across_the_band_the_propulsive_peak_sits_in`.
pub(crate) fn propulsive_peak_index(
    context: &DerivedContext,
    from_index: usize,
    takeoff_index: usize,
) -> Option<usize> {
    plateforce_core::peak::index_of_maximum_over(context.trial.force(), from_index, takeoff_index)
        .ok()
}
