//! Centre of mass kinematics, the boundaries a phase model declares, and the analysis
//! window rules that bound a search.
//!
//! Four rules in the registry place the boundary between unweighting and braking. Two
//! are the same instant reached from different signals and two are not, and which of the
//! four a tool implements is not recoverable from the name it gives the quantity.

use crate::smoothing::{moving_average_boxcar, SmoothingError};
use crate::statistics::{
    index_of_maximum, index_of_minimum, samples_for_duration, DurationRounding,
};

/// Centre of mass acceleration from the net force and the weighed system mass.
///
/// The bodyweight subtraction is what makes this acceleration rather than specific
/// force. One peer reviewed description of this step omits it, and reimplemented from
/// that description every velocity ramps linearly and every height is nonsense.
pub fn center_of_mass_acceleration(
    vertical_ground_reaction_force_newtons: &[f64],
    system_weight_newtons: f64,
    system_mass_kilograms: f64,
) -> Vec<f64> {
    vertical_ground_reaction_force_newtons
        .iter()
        .map(|force| (force - system_weight_newtons) / system_mass_kilograms)
        .collect()
}

/// Running trapezoidal integral, zero at the first sample.
pub fn cumulative_trapezoid(values: &[f64], sample_interval_seconds: f64) -> Vec<f64> {
    let mut integrated = vec![0.0f64; values.len()];
    for index in 1..values.len() {
        integrated[index] = integrated[index - 1]
            + 0.5 * (values[index] + values[index - 1]) * sample_interval_seconds;
    }
    integrated
}

/// Braking start as the most negative centre of mass velocity.
///
/// Searched to the end of an untrimmed recording this finds the landing, where velocity
/// is far more negative than anything in the countermovement.
pub fn braking_start_by_velocity_minimum(
    velocity_meters_per_second: &[f64],
    onset_index: usize,
    search_end_index: usize,
) -> Option<usize> {
    if onset_index + 1 >= search_end_index || search_end_index > velocity_meters_per_second.len() {
        return None;
    }
    index_of_minimum(&velocity_meters_per_second[onset_index..search_end_index])
        .map(|offset| onset_index + offset)
}

/// Which way force is moving through the reference at the boundary.
///
/// Braking begins as force rises back through system weight and propulsion ends as it
/// falls back through it, so the two boundaries are one search read in two directions
/// rather than two rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrossingDirection {
    Rising,
    Falling,
}

/// The sample at which force last sits at or below a reference before rising through it,
/// or first sits at or below it after falling through it.
///
/// The search anchors on the force extremum inside the interval, the minimum for a rising
/// crossing and the maximum for a falling one, so a trace that begins on the wrong side of
/// the reference cannot return its own first sample.
pub fn force_reference_crossing(
    vertical_ground_reaction_force_newtons: &[f64],
    reference_newtons: f64,
    search_start_index: usize,
    search_end_index: usize,
    direction: CrossingDirection,
) -> Option<usize> {
    if search_end_index <= search_start_index
        || search_end_index >= vertical_ground_reaction_force_newtons.len()
    {
        return None;
    }
    let interval = &vertical_ground_reaction_force_newtons[search_start_index..search_end_index];
    let anchor = search_start_index
        + match direction {
            CrossingDirection::Rising => index_of_minimum(interval)?,
            CrossingDirection::Falling => index_of_maximum(interval)?,
        };
    let after_anchor = &vertical_ground_reaction_force_newtons[anchor..=search_end_index];
    let offset = match direction {
        CrossingDirection::Rising => after_anchor
            .iter()
            .rposition(|&force| force <= reference_newtons),
        CrossingDirection::Falling => after_anchor
            .iter()
            .position(|&force| force <= reference_newtons),
    }?;
    Some(anchor + offset)
}

/// Braking start as the last return of force to a reference, searched between the force
/// minimum and the propulsive peak.
///
/// `phase.braking_start.zero_net_force` states the reference as system weight. One
/// reference implementation passes the force at movement onset instead, which is system
/// weight less the bound onset rule's threshold, so its boundary moves when its onset
/// rule moves.
pub fn braking_start_by_force_return(
    vertical_ground_reaction_force_newtons: &[f64],
    onset_index: usize,
    reference_newtons: f64,
    peak_index: usize,
) -> Option<usize> {
    force_reference_crossing(
        vertical_ground_reaction_force_newtons,
        reference_newtons,
        onset_index,
        peak_index,
        CrossingDirection::Rising,
    )
}

/// Propulsion end as the falling crossing of a reference during late propulsion.
pub fn propulsion_end_by_force_crossing(
    vertical_ground_reaction_force_newtons: &[f64],
    reference_newtons: f64,
    braking_start_index: usize,
    takeoff_index: usize,
) -> Option<usize> {
    force_reference_crossing(
        vertical_ground_reaction_force_newtons,
        reference_newtons,
        braking_start_index,
        takeoff_index,
        CrossingDirection::Falling,
    )
}

/// Propulsion end as the instant of maximum centre of mass velocity.
///
/// The mirror of the velocity form of braking start: braking begins at the most negative
/// velocity and propulsion ends at the most positive one, over the same interval.
pub fn propulsion_end_by_velocity_maximum(
    velocity_meters_per_second: &[f64],
    onset_index: usize,
    search_end_index: usize,
) -> Option<usize> {
    if onset_index + 1 >= search_end_index || search_end_index > velocity_meters_per_second.len() {
        return None;
    }
    index_of_maximum(&velocity_meters_per_second[onset_index..search_end_index])
        .map(|offset| onset_index + offset)
}

/// Braking start as the force minimum within a bound.
///
/// Bounded at takeoff this returns the sample before takeoff, because force is still
/// collapsing toward zero and never turns back up. Bounded at the velocity zero
/// crossing it returns the countermovement dip that was intended. One tool computes
/// both and overwrites the first with the second, and only the second reaches its
/// output.
pub fn braking_start_by_force_minimum(
    vertical_ground_reaction_force_newtons: &[f64],
    onset_index: usize,
    search_end_index: usize,
) -> Option<usize> {
    if search_end_index <= onset_index
        || search_end_index > vertical_ground_reaction_force_newtons.len()
    {
        return None;
    }
    index_of_minimum(&vertical_ground_reaction_force_newtons[onset_index..search_end_index])
        .map(|offset| onset_index + offset)
}

/// First upward zero crossing of centre of mass velocity after the velocity minimum.
///
/// The fallback when no crossing exists returns the sample after the minimum, which is
/// the behaviour of the tool this reproduces and is not the same quantity. A caller
/// that cannot tell the two apart is reading a velocity zero that is not one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VelocityZeroCrossing {
    pub index: usize,
    pub is_true_crossing: bool,
}

pub fn velocity_zero_crossing(
    velocity_meters_per_second: &[f64],
    onset_index: usize,
    search_end_index: usize,
) -> Option<VelocityZeroCrossing> {
    velocity_threshold_crossing(
        velocity_meters_per_second,
        onset_index,
        search_end_index,
        0.0,
    )
}

/// The same rising crossing, against a small positive threshold instead of zero.
///
/// A bare zero crossing fires on numerical jitter, and one published rule guards against
/// that by asking for a stated velocity rather than any velocity. The two are one search:
/// the zero form is this one at a threshold of zero, so a caller comparing them is
/// comparing thresholds rather than implementations.
pub fn velocity_threshold_crossing(
    velocity_meters_per_second: &[f64],
    onset_index: usize,
    search_end_index: usize,
    threshold_meters_per_second: f64,
) -> Option<VelocityZeroCrossing> {
    if onset_index + 1 >= search_end_index || search_end_index > velocity_meters_per_second.len() {
        return None;
    }
    let segment = &velocity_meters_per_second[onset_index..search_end_index];
    let minimum = index_of_minimum(segment)?;
    for index in minimum..segment.len().saturating_sub(1) {
        if segment[index] <= threshold_meters_per_second
            && segment[index + 1] > threshold_meters_per_second
        {
            return Some(VelocityZeroCrossing {
                index: onset_index + index + 1,
                is_true_crossing: true,
            });
        }
    }
    if minimum + 1 < segment.len() {
        return Some(VelocityZeroCrossing {
            index: onset_index + minimum + 1,
            is_true_crossing: false,
        });
    }
    None
}

/// The analysis window ends where a smoothed force has fallen a stated percentage below
/// its running maximum since the peak rate of force development.
///
/// The running maximum only rises and the stop compares against a fixed fraction of it,
/// so a higher running maximum can only close the window at the same sample or an earlier
/// one. The smoother is a second one cascaded on whatever conditioning filter already ran,
/// so the effective smoothing on this decision is not the filter setting a user reads.
///
/// `Ok(None)` is the rule running and finding no such fall. The error is the smoother
/// declining, and it names the window it could not fit.
pub fn window_end_by_force_dropoff_from_running_maximum(
    vertical_ground_reaction_force_newtons: &[f64],
    moving_average_seconds: f64,
    sample_rate_hz: f64,
    rounding: DurationRounding,
    dropoff_percent: f64,
    peak_rate_of_force_development_index: usize,
) -> Result<Option<usize>, SmoothingError> {
    let window_samples = samples_for_duration(moving_average_seconds, sample_rate_hz, rounding);
    let smoothed = moving_average_boxcar(vertical_ground_reaction_force_newtons, window_samples)?;
    if peak_rate_of_force_development_index >= smoothed.len() {
        return Ok(None);
    }
    let retained = 1.0 - dropoff_percent / 100.0;
    let mut running_maximum_newtons = smoothed[peak_rate_of_force_development_index];
    for (index, &smoothed_newtons) in smoothed
        .iter()
        .enumerate()
        .skip(peak_rate_of_force_development_index + 1)
    {
        if smoothed_newtons > running_maximum_newtons {
            running_maximum_newtons = smoothed_newtons;
        } else if smoothed_newtons < running_maximum_newtons * retained {
            return Ok(Some(index));
        }
    }
    Ok(None)
}

/// A boundary index and the rule that placed it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlacedLandmark {
    pub index: usize,
    /// The registry id of the rule, in its dotted form, so a phase model consuming this
    /// can report the chain without re-deriving it.
    pub method_id: String,
}

impl PlacedLandmark {
    pub fn new(index: usize, method_id: impl Into<String>) -> Self {
        Self {
            index,
            method_id: method_id.into(),
        }
    }
}

/// The boundaries the phase rules place, kept apart from `Landmarks`.
///
/// `Landmarks` is an input to the scalar jump metrics and holds three fields every one of
/// them reads. These are the output of the phase rules and the input to the phase-anchored
/// ones, so a scalar metric would carry six fields it never reads. Each is optional
/// because each rule can decline.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PhaseLandmarks {
    pub force_minimum: Option<PlacedLandmark>,
    pub braking_start: Option<PlacedLandmark>,
    pub propulsion_start: Option<PlacedLandmark>,
    pub propulsion_end: Option<PlacedLandmark>,
    pub landing_end: Option<PlacedLandmark>,
}

/// The boundaries one phase model declares, in trace order.
///
/// The intervals between them are the model's phases and this type does not name them.
/// What a phase is called is a live question in the field and no registry field carries
/// the answer, so naming them here would settle it in a function body. The model's own id
/// travels in the provenance the caller already builds, for the same reason no other
/// function here mints one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhaseModelBoundaries {
    pub indices: Vec<usize>,
}

/// The single unweighting phase: from the departure below system weight to the return
/// through it.
///
/// The minimum-force instant is not a boundary in this model even though the rule that
/// finds it exists, which is what separates it from the split model. An implementation
/// that puts the minimum in has built the other model.
pub fn phase_model_unweighting_single(
    vertical_ground_reaction_force_newtons: &[f64],
    system_weight_newtons: f64,
    onset_index: usize,
    peak_index: usize,
) -> Option<PhaseModelBoundaries> {
    if peak_index <= onset_index || peak_index >= vertical_ground_reaction_force_newtons.len() {
        return None;
    }
    let start = onset_index
        + vertical_ground_reaction_force_newtons[onset_index..peak_index]
            .iter()
            .position(|&force| force < system_weight_newtons)?;
    let end = force_reference_crossing(
        vertical_ground_reaction_force_newtons,
        system_weight_newtons,
        start,
        peak_index,
        CrossingDirection::Rising,
    )?;
    Some(PhaseModelBoundaries {
        indices: vec![start, end],
    })
}

/// The unloading and yielding split: four boundaries from onset to takeoff.
///
/// The unloading start is this model's own definition of onset, a stated fraction of system
/// weight, and not the bound onset rule's. Reading it from the bound rule would make the
/// model's boundaries move with a choice the model does not make.
#[allow(clippy::too_many_arguments)]
pub fn phase_model_unloading_yielding_split(
    vertical_ground_reaction_force_newtons: &[f64],
    velocity_meters_per_second: &[f64],
    system_weight_newtons: f64,
    unloading_drop_percent_of_system_weight: f64,
    search_start_index: usize,
    peak_index: usize,
    takeoff_index: usize,
) -> Option<PhaseModelBoundaries> {
    if takeoff_index <= search_start_index
        || takeoff_index > vertical_ground_reaction_force_newtons.len()
        || takeoff_index > velocity_meters_per_second.len()
    {
        return None;
    }
    let unloading_level_newtons =
        system_weight_newtons * (1.0 - unloading_drop_percent_of_system_weight / 100.0);
    let unloading_start = search_start_index
        + vertical_ground_reaction_force_newtons[search_start_index..takeoff_index]
            .iter()
            .position(|&force| force < unloading_level_newtons)?;
    // Bounded at the propulsive peak rather than at takeoff, where force is still
    // collapsing toward zero and the minimum is the sample before takeoff.
    let force_minimum = braking_start_by_force_minimum(
        vertical_ground_reaction_force_newtons,
        unloading_start,
        peak_index,
    )?;
    let velocity_minimum = braking_start_by_velocity_minimum(
        velocity_meters_per_second,
        force_minimum,
        takeoff_index,
    )?;
    let positive_velocity = velocity_threshold_crossing(
        velocity_meters_per_second,
        velocity_minimum,
        takeoff_index,
        0.0,
    )?;
    Some(PhaseModelBoundaries {
        indices: vec![
            unloading_start,
            force_minimum,
            velocity_minimum,
            positive_velocity.index,
            takeoff_index,
        ],
    })
}

/// The interval over which force exceeds system weight, from the rising crossing after the
/// unweighting minimum to the falling crossing during late propulsion.
///
/// Free once the two crossings exist, and it is not a dual-plate rule despite sitting under
/// the window the asymmetry entries declare.
pub fn positive_impulse_window(
    vertical_ground_reaction_force_newtons: &[f64],
    system_weight_newtons: f64,
    onset_index: usize,
    peak_index: usize,
    takeoff_index: usize,
) -> Option<(usize, usize)> {
    let start = force_reference_crossing(
        vertical_ground_reaction_force_newtons,
        system_weight_newtons,
        onset_index,
        peak_index,
        CrossingDirection::Rising,
    )?;
    let end = force_reference_crossing(
        vertical_ground_reaction_force_newtons,
        system_weight_newtons,
        start,
        takeoff_index,
        CrossingDirection::Falling,
    )?;
    Some((start, end))
}

/// The propulsion phase split at a stated fraction of its duration.
pub fn propulsion_subdivision_by_time(
    propulsion_start_index: usize,
    propulsion_end_index: usize,
    split_percent_of_duration: f64,
) -> Option<usize> {
    if propulsion_end_index <= propulsion_start_index {
        return None;
    }
    let duration = (propulsion_end_index - propulsion_start_index) as f64;
    let offset = (duration * split_percent_of_duration / 100.0).round() as usize;
    Some(propulsion_start_index + offset)
}

/// The propulsion phase split where force descends through system weight.
pub fn propulsion_subdivision_by_force_crossing(
    vertical_ground_reaction_force_newtons: &[f64],
    system_weight_newtons: f64,
    propulsion_start_index: usize,
    propulsion_end_index: usize,
) -> Option<usize> {
    force_reference_crossing(
        vertical_ground_reaction_force_newtons,
        system_weight_newtons,
        propulsion_start_index,
        propulsion_end_index,
        CrossingDirection::Falling,
    )
}

/// Where the landing ended, or why it did not.
#[derive(Debug, Clone, PartialEq)]
pub enum LandingEnd {
    /// The first frame at which the centre of mass had stopped descending.
    Settled { index: usize },
    /// The recording ran out while the centre of mass was still moving. One shipped
    /// implementation returns no landing metrics at all in this case rather than degraded
    /// ones, which is a gap a reader cannot see; this names it instead.
    RecordingEndsWhileStillMoving {
        last_index: usize,
        velocity_meters_per_second: f64,
    },
    /// The series handed in was not the one this rule describes. Its initial value has to be
    /// pinned at touchdown, because a landing velocity integrated from anywhere else is a
    /// different quantity that happens to share a unit.
    NotAnchoredAtTouchdown { touchdown_index: usize },
}

/// Landing ends where the centre of mass stops descending, read off a series pinned to the
/// negative takeoff velocity at touchdown.
///
/// The anchor is checked rather than assumed. Nothing here decides how the series was made,
/// but this rule is only meaningful on one kind of series, and accepting any other would
/// return a number under this rule's name that this rule did not produce.
pub fn landing_end_by_zero_com_velocity(
    velocity: &crate::series::VelocitySeries,
    touchdown_index: usize,
) -> LandingEnd {
    let anchored_here = matches!(
        velocity.spec().anchor,
        crate::series::IntegrationAnchor::SinglePointAtValue { index, .. } if index == touchdown_index
    );
    if !anchored_here {
        return LandingEnd::NotAnchoredAtTouchdown { touchdown_index };
    }
    let samples = velocity.meters_per_second();
    if touchdown_index >= samples.len() {
        return LandingEnd::NotAnchoredAtTouchdown { touchdown_index };
    }
    for (offset, value) in samples[touchdown_index..].iter().enumerate() {
        if *value >= 0.0 {
            return LandingEnd::Settled {
                index: touchdown_index + offset,
            };
        }
    }
    LandingEnd::RecordingEndsWhileStillMoving {
        last_index: samples.len() - 1,
        velocity_meters_per_second: samples[samples.len() - 1],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integrating_a_constant_acceleration_gives_a_linear_velocity() {
        let acceleration = vec![2.0; 1001];
        let velocity = cumulative_trapezoid(&acceleration, 0.001);
        assert!((velocity[1000] - 2.0).abs() < 1e-12, "{}", velocity[1000]);
        assert_eq!(velocity[0], 0.0);
    }

    #[test]
    fn a_velocity_minimum_search_that_runs_past_takeoff_finds_the_landing() {
        let mut velocity = vec![0.0; 100];
        velocity.extend((0..100).map(|i| -(i as f64) / 100.0));
        velocity.extend((0..100).map(|i| -1.0 + (i as f64) / 50.0));
        let takeoff = velocity.len();
        velocity.extend((0..200).map(|i| -5.0 - (i as f64) / 100.0));
        let bounded = braking_start_by_velocity_minimum(&velocity, 0, takeoff).unwrap();
        let unbounded = braking_start_by_velocity_minimum(&velocity, 0, velocity.len()).unwrap();
        assert_eq!(bounded, 200);
        assert!(
            unbounded > takeoff,
            "unbounded search stayed before takeoff"
        );
    }

    #[test]
    fn a_fallback_velocity_zero_is_flagged_as_not_a_crossing() {
        let mut velocity: Vec<f64> = (0..200).map(|i| -(i as f64) / 100.0).collect();
        velocity.extend(std::iter::repeat_n(-2.0, 50));
        let never_returns = velocity_zero_crossing(&velocity, 0, velocity.len()).unwrap();
        assert!(
            !never_returns.is_true_crossing,
            "a fall that never returns reported a crossing"
        );

        let mut recovers = velocity.clone();
        recovers.extend((0..50).map(|i| -2.0 + (i as f64) / 10.0));
        let crossed = velocity_zero_crossing(&recovers, 0, recovers.len()).unwrap();
        assert!(crossed.is_true_crossing);
    }

    #[test]
    fn a_force_minimum_bounded_at_takeoff_returns_the_sample_before_takeoff() {
        let mut force = vec![600.0; 100];
        force.extend((0..100).map(|i| 400.0 + i as f64));
        force.extend((0..100).map(|i| 500.0 - 5.0 * i as f64));
        let takeoff = force.len();
        let at_takeoff = braking_start_by_force_minimum(&force, 0, takeoff).unwrap();
        let at_dip = braking_start_by_force_minimum(&force, 0, 150).unwrap();
        assert_eq!(at_takeoff, takeoff - 1);
        assert_eq!(at_dip, 100);
    }

    /// A rise and a fall through one reference are the same search read two ways, so the
    /// two boundaries land on the two sides of one excursion above the reference. Each
    /// direction is bounded the way its landmark is: the rise at the propulsive peak, the
    /// fall at takeoff.
    #[test]
    fn one_crossing_search_returns_braking_start_rising_and_propulsion_end_falling() {
        let weight = 700.0;
        let mut force = vec![weight; 200];
        force.extend((0..100).map(|i| weight - 3.0 * i as f64));
        force.extend((0..100).map(|i| weight - 300.0 + 9.0 * i as f64));
        force.extend((0..100).map(|i| weight + 600.0 - 12.0 * i as f64));
        force.extend(vec![weight - 600.0; 100]);
        let peak = 400;
        let takeoff = force.len() - 1;

        let rising =
            force_reference_crossing(&force, weight, 0, peak, CrossingDirection::Rising).unwrap();
        let falling =
            force_reference_crossing(&force, weight, rising, takeoff, CrossingDirection::Falling)
                .unwrap();
        assert!(
            rising < falling,
            "rising {rising} against falling {falling}"
        );
        assert!(
            force[rising] <= weight && force[rising + 1] > weight,
            "{rising}"
        );
        assert!(
            force[falling] <= weight && force[falling - 1] > weight,
            "{falling}"
        );
    }

    /// The generalised search has to reproduce the wrapper the conformance harness calls,
    /// or one landmark has two implementations.
    #[test]
    fn the_braking_start_wrapper_is_the_rising_crossing() {
        let weight = 700.0;
        let mut force = vec![weight; 200];
        force.extend((0..100).map(|i| weight - 3.0 * i as f64));
        force.extend((0..200).map(|i| weight - 300.0 + 9.0 * i as f64));
        let peak = force.len() - 1;
        assert_eq!(
            braking_start_by_force_return(&force, 0, weight, peak),
            force_reference_crossing(&force, weight, 0, peak, CrossingDirection::Rising)
        );
    }

    /// Velocity argmin and argmax are the two identities the registry mirrors across the
    /// braking and propulsion boundaries.
    #[test]
    fn the_velocity_extrema_are_the_two_boundaries_of_one_interval() {
        let mut velocity = vec![0.0; 100];
        velocity.extend((0..100).map(|i| -(i as f64) / 100.0));
        velocity.extend((0..200).map(|i| -1.0 + (i as f64) / 100.0));
        let end = velocity.len();
        let braking = braking_start_by_velocity_minimum(&velocity, 0, end).unwrap();
        let propulsion = propulsion_end_by_velocity_maximum(&velocity, 0, end).unwrap();
        assert_eq!(braking, 200);
        assert_eq!(propulsion, end - 1);
        assert!(braking < propulsion);
    }

    /// A plateau after the peak, then a decay, with a stated sample rate so the window is
    /// a duration rather than a count.
    fn propulsive_rise_then_decay() -> Vec<f64> {
        let mut force = vec![700.0f64; 600];
        force.extend((0..600).map(|i| 700.0 + 2.0 * i as f64));
        force.extend((0..1200).map(|i| 1900.0 - 1.5 * i as f64));
        force
    }

    #[test]
    fn the_window_closes_where_the_smoothed_force_has_fallen_the_stated_percentage() {
        let force = propulsive_rise_then_decay();
        let end = window_end_by_force_dropoff_from_running_maximum(
            &force,
            0.1,
            1200.0,
            DurationRounding::Nearest,
            5.0,
            600,
        )
        .unwrap()
        .expect("the trace falls far enough for the rule to close the window");
        assert!(end > 1200 && end < force.len(), "{end}");
    }

    /// The running maximum only rises, and the stop compares against a fixed fraction of
    /// it, so a higher running maximum raises the level the trace has to fall below and can
    /// only close the window at the same sample or an earlier one. A single high sample
    /// after the peak therefore shortens the window rather than extending it.
    #[test]
    fn a_single_high_sample_after_the_peak_closes_the_window_earlier() {
        let force = propulsive_rise_then_decay();
        let mut spiked = force.clone();
        for value in spiked.iter_mut().skip(1210).take(1) {
            *value = 6000.0;
        }
        let plain = window_end_by_force_dropoff_from_running_maximum(
            &force,
            0.02,
            1200.0,
            DurationRounding::Nearest,
            5.0,
            600,
        )
        .unwrap()
        .unwrap();
        let spiked_end = window_end_by_force_dropoff_from_running_maximum(
            &spiked,
            0.02,
            1200.0,
            DurationRounding::Nearest,
            5.0,
            600,
        )
        .unwrap()
        .unwrap();
        assert!(
            spiked_end < plain,
            "one high sample left the window at {spiked_end} against {plain}"
        );
    }

    /// The width is a duration and is converted, so the error names the sample count the
    /// rate produced rather than the number the caller wrote.
    #[test]
    fn the_moving_average_width_is_read_in_seconds_and_converted() {
        let force = vec![700.0f64; 100];
        let refused = window_end_by_force_dropoff_from_running_maximum(
            &force,
            0.1,
            1200.0,
            DurationRounding::Nearest,
            5.0,
            0,
        )
        .expect_err("a 0.1 second window does not fit 100 samples");
        assert!(
            matches!(
                refused,
                SmoothingError::WindowLongerThanTrace {
                    window_length: 120,
                    sample_count: 100
                }
            ),
            "{refused}"
        );

        let long = propulsive_rise_then_decay();
        let at_1200 = window_end_by_force_dropoff_from_running_maximum(
            &long,
            0.1,
            1200.0,
            DurationRounding::Nearest,
            5.0,
            600,
        )
        .unwrap();
        let at_600 = window_end_by_force_dropoff_from_running_maximum(
            &long,
            0.1,
            600.0,
            DurationRounding::Nearest,
            5.0,
            600,
        )
        .unwrap();
        assert_ne!(at_1200, at_600, "the sample rate did not reach the window");
    }

    const SYSTEM_WEIGHT_NEWTONS: f64 = 700.0;
    const TAKEOFF_INDEX: usize = 1300;
    const PEAK_INDEX: usize = 1100;

    /// A countermovement shape with a real unweighting dip, a braking rise through system
    /// weight, a propulsive peak and a fall to zero, so both phase models resolve on it.
    fn countermovement_force() -> Vec<f64> {
        let mut force = vec![SYSTEM_WEIGHT_NEWTONS; 500];
        force.extend((0..300).map(|i| SYSTEM_WEIGHT_NEWTONS - 400.0 * i as f64 / 300.0));
        force.extend((0..300).map(|i| 300.0 + 1100.0 * i as f64 / 300.0));
        force.extend((0..200).map(|i| 1400.0 - 1400.0 * i as f64 / 200.0));
        force.extend(vec![0.0; 300]);
        force
    }

    fn countermovement_velocity(force: &[f64]) -> Vec<f64> {
        let mass = SYSTEM_WEIGHT_NEWTONS / 9.806_65;
        let acceleration = center_of_mass_acceleration(force, SYSTEM_WEIGHT_NEWTONS, mass);
        cumulative_trapezoid(&acceleration, 1.0 / 1200.0)
    }

    #[test]
    fn the_threshold_form_of_the_velocity_crossing_is_the_zero_form_at_zero() {
        let force = countermovement_force();
        let velocity = countermovement_velocity(&force);
        assert_eq!(
            velocity_zero_crossing(&velocity, 500, TAKEOFF_INDEX),
            velocity_threshold_crossing(&velocity, 500, TAKEOFF_INDEX, 0.0)
        );
        let guarded = velocity_threshold_crossing(&velocity, 500, TAKEOFF_INDEX, 0.01).unwrap();
        let bare = velocity_zero_crossing(&velocity, 500, TAKEOFF_INDEX).unwrap();
        assert!(guarded.index > bare.index, "{guarded:?} against {bare:?}");
    }

    /// The minimum-force instant is not a boundary in this model, which is the whole
    /// difference between it and the split model.
    #[test]
    fn the_single_unweighting_model_does_not_declare_the_force_minimum() {
        let force = countermovement_force();
        let model =
            phase_model_unweighting_single(&force, SYSTEM_WEIGHT_NEWTONS, 500, PEAK_INDEX).unwrap();
        let minimum = braking_start_by_force_minimum(&force, 500, PEAK_INDEX).unwrap();
        assert_eq!(model.indices.len(), 2, "{model:?}");
        assert!(
            !model.indices.contains(&minimum),
            "the model declared the force minimum at {minimum}: {model:?}"
        );
        assert!(model.indices[0] < minimum && minimum < model.indices[1]);
    }

    /// The split model reads its own unloading definition, so a caller passing a different
    /// bound onset cannot move its first boundary.
    #[test]
    fn the_split_models_unloading_start_does_not_follow_the_bound_onset_rule() {
        let force = countermovement_force();
        let velocity = countermovement_velocity(&force);
        let from_early = phase_model_unloading_yielding_split(
            &force,
            &velocity,
            SYSTEM_WEIGHT_NEWTONS,
            2.5,
            0,
            PEAK_INDEX,
            TAKEOFF_INDEX,
        )
        .unwrap();
        let from_late = phase_model_unloading_yielding_split(
            &force,
            &velocity,
            SYSTEM_WEIGHT_NEWTONS,
            2.5,
            480,
            PEAK_INDEX,
            TAKEOFF_INDEX,
        )
        .unwrap();
        assert_eq!(from_early.indices, from_late.indices);
        assert_eq!(from_early.indices.len(), 5, "{from_early:?}");
        assert!(
            from_early.indices.windows(2).all(|pair| pair[0] < pair[1]),
            "{from_early:?}"
        );

        let deeper = phase_model_unloading_yielding_split(
            &force,
            &velocity,
            SYSTEM_WEIGHT_NEWTONS,
            10.0,
            0,
            PEAK_INDEX,
            TAKEOFF_INDEX,
        )
        .unwrap();
        assert!(
            deeper.indices[0] > from_early.indices[0],
            "a deeper drop started no later: {deeper:?} against {from_early:?}"
        );
    }

    /// The window is the interval between the two crossings, so its ends are the two
    /// landmarks rather than a third derivation of them.
    #[test]
    fn the_positive_impulse_window_ends_on_the_two_crossings() {
        let force = countermovement_force();
        let (start, end) = positive_impulse_window(
            &force,
            SYSTEM_WEIGHT_NEWTONS,
            500,
            PEAK_INDEX,
            TAKEOFF_INDEX,
        )
        .unwrap();
        assert_eq!(
            start,
            force_reference_crossing(
                &force,
                SYSTEM_WEIGHT_NEWTONS,
                500,
                PEAK_INDEX,
                CrossingDirection::Rising
            )
            .unwrap()
        );
        assert_eq!(
            end,
            propulsion_end_by_force_crossing(&force, SYSTEM_WEIGHT_NEWTONS, start, TAKEOFF_INDEX)
                .unwrap()
        );
        assert!(force[start] <= SYSTEM_WEIGHT_NEWTONS && force[start + 1] > SYSTEM_WEIGHT_NEWTONS);
        assert!(force[end] <= SYSTEM_WEIGHT_NEWTONS && force[end - 1] > SYSTEM_WEIGHT_NEWTONS);
    }

    #[test]
    fn the_two_propulsion_subdivisions_split_the_same_phase_at_two_instants() {
        let force = countermovement_force();
        let by_time = propulsion_subdivision_by_time(900, 1300, 50.0).unwrap();
        assert_eq!(by_time, 1100);
        let by_force =
            propulsion_subdivision_by_force_crossing(&force, SYSTEM_WEIGHT_NEWTONS, 900, 1300)
                .unwrap();
        assert!((900..1300).contains(&by_force), "{by_force}");
        assert_ne!(by_time, by_force);
        assert!(propulsion_subdivision_by_time(1300, 900, 50.0).is_none());
    }

    #[test]
    fn a_placed_landmark_carries_the_rule_that_placed_it() {
        let landmarks = PhaseLandmarks {
            braking_start: Some(PlacedLandmark::new(
                812,
                "phase.braking_start.zero_net_force",
            )),
            ..PhaseLandmarks::default()
        };
        let placed = landmarks.braking_start.as_ref().unwrap();
        assert_eq!(placed.index, 812);
        assert_eq!(placed.method_id, "phase.braking_start.zero_net_force");
        assert!(landmarks.propulsion_end.is_none());
    }

    mod landing {
        use super::*;
        use crate::series::{
            centre_of_mass_velocity_meters_per_second, IntegrationAnchor, IntegrationDirection,
            IntegrationSpec, IntegrationStart, QuadratureRule, VelocitySeries,
        };
        use crate::trial::WeighingEpoch;
        use crate::Trial;

        const WEIGHT: f64 = 700.0;
        const RATE_HZ: f64 = 1000.0;

        fn epoch() -> WeighingEpoch {
            WeighingEpoch {
                start_index: 0,
                end_index: 100,
                system_weight_newtons: WEIGHT,
                standard_deviation_newtons: 1.0,
                tied_window_count: 1,
                tied_weight_low_newtons: WEIGHT,
                tied_weight_high_newtons: WEIGHT,
            }
        }

        /// Quiet standing, then flight, then a landing that pushes back hard enough to stop
        /// the descent inside the recording.
        fn landing_trace(landing_samples: usize, landing_force: f64) -> (Vec<f64>, usize) {
            let mut force = vec![WEIGHT; 200];
            force.extend(vec![0.0; 300]);
            let touchdown = force.len();
            force.extend(vec![landing_force; landing_samples]);
            (force, touchdown)
        }

        fn series_anchored_at(force: &[f64], touchdown: usize, value: f64) -> VelocitySeries {
            let trial = Trial::new(force.to_vec(), RATE_HZ).unwrap();
            // The integral runs from the trial start and the constant is pinned at touchdown,
            // which is the same curve after touchdown as integrating the landing alone.
            let spec = IntegrationSpec {
                quadrature: QuadratureRule::Trapezoid,
                direction: IntegrationDirection::Forward,
                start: IntegrationStart::TrialStart,
                anchor: IntegrationAnchor::SinglePointAtValue {
                    index: touchdown,
                    value,
                    stated_by_method_id: "phase.landing_end.zero_com_velocity".to_string(),
                },
            };
            centre_of_mass_velocity_meters_per_second(&trial, &epoch(), &spec, 9.81)
        }

        #[test]
        fn the_landing_ends_where_the_descent_stops() {
            let (force, touchdown) = landing_trace(600, WEIGHT * 3.0);
            let velocity = series_anchored_at(&force, touchdown, -2.0);
            let LandingEnd::Settled { index } =
                landing_end_by_zero_com_velocity(&velocity, touchdown)
            else {
                panic!("the descent did not stop inside the recording")
            };
            assert!(index > touchdown, "the landing ended before it began");
            assert!(velocity.at(index).unwrap() >= 0.0);
            assert!(velocity.at(index - 1).unwrap() < 0.0, "not the first frame");
        }

        #[test]
        fn a_recording_that_ends_mid_descent_is_named_rather_than_left_empty() {
            // The same landing cut short. One shipped implementation returns no landing
            // metrics here and says nothing about why.
            let (force, touchdown) = landing_trace(40, WEIGHT * 3.0);
            let velocity = series_anchored_at(&force, touchdown, -2.0);
            let outcome = landing_end_by_zero_com_velocity(&velocity, touchdown);
            let LandingEnd::RecordingEndsWhileStillMoving {
                velocity_meters_per_second,
                ..
            } = outcome
            else {
                panic!("a truncated landing was reported as settled: {outcome:?}")
            };
            assert!(velocity_meters_per_second < 0.0);
        }

        #[test]
        fn a_series_pinned_somewhere_else_is_refused_rather_than_read() {
            // Without this the rule would accept any velocity series and return an index
            // under its own name that its own arithmetic never produced.
            let (force, touchdown) = landing_trace(600, WEIGHT * 3.0);
            let trial = Trial::new(force.clone(), RATE_HZ).unwrap();
            let spec = IntegrationSpec {
                quadrature: QuadratureRule::Trapezoid,
                direction: IntegrationDirection::Forward,
                start: IntegrationStart::TrialStart,
                anchor: IntegrationAnchor::SinglePoint { index: 0 },
            };
            let elsewhere =
                centre_of_mass_velocity_meters_per_second(&trial, &epoch(), &spec, 9.81);
            assert_eq!(
                landing_end_by_zero_com_velocity(&elsewhere, touchdown),
                LandingEnd::NotAnchoredAtTouchdown {
                    touchdown_index: touchdown
                }
            );

            // And pinned at the right value but the wrong instant is the same refusal.
            let wrong_instant = series_anchored_at(&force, touchdown - 50, -2.0);
            assert_eq!(
                landing_end_by_zero_com_velocity(&wrong_instant, touchdown),
                LandingEnd::NotAnchoredAtTouchdown {
                    touchdown_index: touchdown
                }
            );
        }

        #[test]
        fn a_harder_landing_stops_the_descent_sooner() {
            let (soft, touchdown) = landing_trace(600, WEIGHT * 2.0);
            let (hard, _) = landing_trace(600, WEIGHT * 5.0);
            let stop = |force: &[f64]| match landing_end_by_zero_com_velocity(
                &series_anchored_at(force, touchdown, -2.0),
                touchdown,
            ) {
                LandingEnd::Settled { index } => index,
                other => panic!("{other:?}"),
            };
            assert!(
                stop(&hard) < stop(&soft),
                "the landing force changed nothing: {} against {}",
                stop(&hard),
                stop(&soft)
            );
        }
    }
}
