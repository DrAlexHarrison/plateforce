//! What a phase boundary rule reads before it can place anything: which landmarks it is
//! missing, the centre-of-mass velocity curve, and the bound a crossing search runs to.
//!
//! `integration.toml` states four choices on constructs of their own: the quadrature the
//! net-force integral is evaluated by, the direction it runs, where it starts, and where the
//! constant is pinned. No rule fills any of the four, so a rule that reads a velocity states
//! all four here rather than each rule stating its own, and the four ids are written into
//! that rule's bound values under `ParameterSource::Assumed`. A boundary that moved with an
//! integration setting no fingerprint carried would be a silent default.
//!
//! The forward spec is the one `takeoff_velocity_meters_per_second` reads its impulse-momentum
//! identity off, so a velocity landmark and the takeoff velocity beside it come from one curve.

use std::collections::BTreeMap;

use plateforce_core::phases::{BoundedCrossing, PhaseModelOutcome};

use crate::derived::{DerivedContext, DerivedOutcome};
use crate::resolution::{BoundValues, RuleRefusal};
use crate::response::Quantity;

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

/// A rule that divides an interval in two, as the instant it placed inside that interval or
/// the refusal that says it placed none.
///
/// A split equal to either end divides the interval into all of it and none of it, so a
/// sub-phase metric taken against it is the whole phase or nothing while its key still reads
/// as a split. `is_true_crossing` cannot catch that: the comparison behind it is against the
/// search anchor, and a split at the interval's far end sits past the anchor.
///
/// The bound is read here rather than by each rule, so a split placed by a share of the
/// duration and a split placed by a crossing are held to one definition of inside.
pub(crate) fn subdivision_outcome(
    context: &DerivedContext,
    method_id: &str,
    key: &'static str,
    name: &'static str,
    interval: (usize, usize),
    index: Option<usize>,
    bound: BoundValues,
) -> DerivedOutcome {
    let (start_index, end_index) = interval;
    match index {
        Some(index) if index > start_index && index < end_index => {
            placed_outcome(context, key, name, Some(index), bound)
        }
        Some(index) => DerivedOutcome::declined(
            bound,
            RuleRefusal::Refused(Box::new(
                plateforce_core::Refusal::subdivision_outside_its_interval(
                    method_id,
                    context.trial.time_at(index),
                    context.trial.time_at(start_index),
                    context.trial.time_at(end_index),
                ),
            )),
        ),
        None => placed_outcome(context, key, name, None, bound),
    }
}

/// What a crossing subdivision searched: the interval it divides and the reference it
/// compared each sample in that interval against.
///
/// The three travel together because the refusal needs all three, and a search that qualified
/// nothing is only readable beside the interval and the reference that qualified nothing in it.
pub(crate) struct SearchedInterval {
    pub start_index: usize,
    pub end_index: usize,
    pub reference_newtons: f64,
}

/// The same as `subdivision_outcome`, for a subdivision searched for as a crossing of a force
/// reference.
///
/// The refusal for a search that qualified nothing carries the interval it read and the
/// reference it compared against, and counts the samples it read rather than reporting none.
/// A rule handed an interval that never descends to the reference has read every sample in it
/// and found no candidate, which is a fact about the interval those bounds enclose. Reporting
/// zero candidates would read as an empty recording.
pub(crate) fn subdivision_crossing_outcome(
    context: &DerivedContext,
    method_id: &str,
    key: &'static str,
    name: &'static str,
    searched: SearchedInterval,
    crossing: Option<BoundedCrossing>,
    bound: BoundValues,
) -> DerivedOutcome {
    let SearchedInterval {
        start_index,
        end_index,
        reference_newtons,
    } = searched;
    match crossing {
        Some(crossing) if crossing.is_true_crossing => subdivision_outcome(
            context,
            method_id,
            key,
            name,
            (start_index, end_index),
            Some(crossing.index),
            bound,
        ),
        Some(fallback) => declined_at_a_fallback(context, method_id, bound, fallback),
        None => DerivedOutcome::declined(
            bound,
            RuleRefusal::Refused(Box::new(plateforce_core::Refusal::nothing_qualified(
                method_id,
                (end_index + 1).saturating_sub(start_index),
                BTreeMap::from([
                    (
                        "interval_start_seconds".to_string(),
                        context.trial.time_at(start_index),
                    ),
                    (
                        "interval_end_seconds".to_string(),
                        context.trial.time_at(end_index),
                    ),
                    ("reference_newtons".to_string(), reference_newtons),
                ]),
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

/// A phase model's boundaries, or the refusal that says which of them the recording did not
/// carry.
///
/// A model declines whole rather than publishing the boundaries below the one it could not
/// place, because the intervals between them are what the model asserts and an interval with
/// an unmet end is not one of them.
///
/// The keys are read off the quantities the entry publishes rather than passed as a list of
/// their own. A second list is a subset of the first the moment a model gains a boundary, and
/// the zip below would have placed the shorter of the two and reported nothing about the rest:
/// a quantity the registry publishes, the interface draws a column for, and no run ever fills.
pub(crate) fn model_outcome(
    context: &DerivedContext,
    method_id: &str,
    quantities: &[Quantity],
    outcome: PhaseModelOutcome,
    bound: BoundValues,
) -> DerivedOutcome {
    match outcome {
        // Over the quantities rather than over the indices, so a model that placed fewer
        // boundaries than its entry publishes reports the rest as quantities with no value
        // instead of leaving them out of the answer altogether. A reader can see a blank cell;
        // nobody can see a key that never arrived.
        PhaseModelOutcome::Placed(model) => DerivedOutcome {
            values: quantities
                .iter()
                .enumerate()
                .map(|(position, quantity)| {
                    (
                        quantity.key,
                        model
                            .indices
                            .get(position)
                            .map(|index| context.trial.time_at(*index)),
                    )
                })
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
            values: quantities
                .iter()
                .map(|quantity| (quantity.key, None))
                .collect(),
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

/// A phase a rule can run across, named by the two constructs that bound it.
///
/// Three, because three are what one request can name. Every other published sub-phase of the
/// countermovement has both of its ends on one construct, and a request carries one rule per
/// construct: Harry's eccentric yielding phase runs from the force minimum to peak negative
/// centre of mass velocity, and those are two `braking_phase_start` rules, so a request naming
/// one cannot also place the other.
///
/// Which named phase each of these is depends on the rules bound to its two ends, so the values
/// name the boundaries rather than one school's word for the interval between them. With the
/// braking phase starting at the force minimum, the first is Harry's unloading phase; starting
/// at peak negative centre of mass velocity, it is McMahon's unweighting phase.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Phase {
    OnsetToBrakingStart,
    BrakingStartToPropulsionStart,
    PropulsionStartToPropulsionEnd,
}

/// The name a caller states to choose one, and the keys it takes.
///
/// One home for both, so a rule reading the name, a check answering it and the registry rows
/// publishing it cannot drift apart.
pub(crate) const PHASE_PARAMETER: &str = "phase";

pub(crate) const PHASE_VALUES: &[(&str, Phase)] = &[
    ("onset_to_braking_start", Phase::OnsetToBrakingStart),
    (
        "braking_start_to_propulsion_start",
        Phase::BrakingStartToPropulsionStart,
    ),
    (
        "propulsion_start_to_propulsion_end",
        Phase::PropulsionStartToPropulsionEnd,
    ),
];

/// The samples one phase runs between, or the constructs whose rules placed no boundary for it.
///
/// Both ends are asked for whichever is missing, so the chain records that the rule read them
/// and a refusal names only what is actually absent.
pub(crate) fn phase_interval(
    context: &DerivedContext,
    phase: Phase,
) -> Result<(usize, usize), Vec<&'static str>> {
    use crate::slots::{braking_phase_start, propulsion_phase_end, propulsion_phase_start};

    let ((from_construct, from), (to_construct, to)) = match phase {
        Phase::OnsetToBrakingStart => (
            (crate::binding::ONSET_CONSTRUCT, context.onset_index()),
            (
                braking_phase_start::CONSTRUCT,
                braking_phase_start::placed(context),
            ),
        ),
        Phase::BrakingStartToPropulsionStart => (
            (
                braking_phase_start::CONSTRUCT,
                braking_phase_start::placed(context),
            ),
            (
                propulsion_phase_start::CONSTRUCT,
                propulsion_phase_start::placed(context),
            ),
        ),
        Phase::PropulsionStartToPropulsionEnd => (
            (
                propulsion_phase_start::CONSTRUCT,
                propulsion_phase_start::placed(context),
            ),
            (
                propulsion_phase_end::CONSTRUCT,
                propulsion_phase_end::placed(context),
            ),
        ),
    };
    match (from, to) {
        (Some(from), Some(to)) if to > from => Ok((from, to)),
        (from, to) => {
            let mut missing = Vec::new();
            if from.is_none() {
                missing.push(from_construct);
            }
            if to.is_none() {
                missing.push(to_construct);
            }
            // Both placed and out of order is a phase of no duration rather than a missing
            // boundary, and the end is the one a caller moves to fix it.
            if missing.is_empty() {
                missing.push(to_construct);
            }
            Err(missing)
        }
    }
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
