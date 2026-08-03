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

use plateforce_core::phases::VelocityZeroCrossing;

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
    crossing: Option<VelocityZeroCrossing>,
    bound: BoundValues,
) -> DerivedOutcome {
    match crossing {
        Some(crossing) if crossing.is_true_crossing => {
            placed_outcome(context, key, name, Some(crossing.index), bound)
        }
        Some(fallback) => DerivedOutcome::declined(
            bound,
            RuleRefusal::Refused(Box::new(plateforce_core::Refusal::nothing_qualified(
                method_id,
                1,
                BTreeMap::from([("fallback_index".to_string(), fallback.index as f64)]),
            ))),
        ),
        None => placed_outcome(context, key, name, None, bound),
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
            crate::binding::ONSET_CONSTRUCT => context.onset_index.is_none(),
            crate::binding::TAKEOFF_CONSTRUCT => context.takeoff_index.is_none(),
            LANDING_CONSTRUCT => context.touchdown_index.is_none(),
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
