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
    if onset_index + 1 >= search_end_index || search_end_index > velocity_meters_per_second.len() {
        return None;
    }
    let segment = &velocity_meters_per_second[onset_index..search_end_index];
    let minimum = index_of_minimum(segment)?;
    for index in minimum..segment.len().saturating_sub(1) {
        if segment[index] <= 0.0 && segment[index + 1] > 0.0 {
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
/// The running maximum only rises, so one high sample after the peak raises the level the
/// trace then has to fall below and carries the window past it. The smoother is a second
/// one cascaded on whatever conditioning filter already ran, so the effective smoothing on
/// this decision is not the filter setting a user reads.
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
    for index in peak_rate_of_force_development_index + 1..smoothed.len() {
        if smoothed[index] > running_maximum_newtons {
            running_maximum_newtons = smoothed[index];
        } else if smoothed[index] < running_maximum_newtons * retained {
            return Ok(Some(index));
        }
    }
    Ok(None)
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
}
