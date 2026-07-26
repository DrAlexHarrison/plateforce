//! Centre of mass kinematics, and the boundary between unweighting and braking.
//!
//! Four rules in the registry place that boundary. Two are the same instant reached
//! from different signals and two are not, and which of the four a tool implements is
//! not recoverable from the name it gives the quantity.

use crate::statistics::index_of_minimum;

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
/// The upper bound is not optional. Searched to the end of an untrimmed recording this
/// finds the landing, where velocity is far more negative than anything in the
/// countermovement.
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

/// Braking start as the last return of force to its value at movement onset, searched
/// between the force minimum and the propulsive peak.
pub fn braking_start_by_force_return(
    vertical_ground_reaction_force_newtons: &[f64],
    onset_index: usize,
    reference_newtons: f64,
    peak_index: usize,
) -> Option<usize> {
    if peak_index <= onset_index || peak_index >= vertical_ground_reaction_force_newtons.len() {
        return None;
    }
    let minimum = onset_index
        + index_of_minimum(&vertical_ground_reaction_force_newtons[onset_index..peak_index])?;
    vertical_ground_reaction_force_newtons[minimum..=peak_index]
        .iter()
        .rposition(|&force| force <= reference_newtons)
        .map(|offset| minimum + offset)
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
        assert!(unbounded > takeoff, "unbounded search stayed before takeoff");
    }

    #[test]
    fn a_fallback_velocity_zero_is_flagged_as_not_a_crossing() {
        let mut velocity: Vec<f64> = (0..200).map(|i| -(i as f64) / 100.0).collect();
        velocity.extend(std::iter::repeat(-2.0).take(50));
        let never_returns = velocity_zero_crossing(&velocity, 0, velocity.len()).unwrap();
        assert!(!never_returns.is_true_crossing, "a fall that never returns reported a crossing");

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
}
