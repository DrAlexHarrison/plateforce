//! What the plate reported when nothing stood on it, and the drift policy that puts the
//! velocity back where it found it.
//!
//! Nothing here decides a method. A caller passes the window a bound rule resolved and the
//! landmarks the bound landmark rules placed.

use crate::statistics::{mean, CompensatedAccumulator};

/// The plate's own reading over a window in which it carried nothing, which is the flight
/// phase of a jump. Nothing when the window holds no samples to average.
pub fn unloaded_epoch_offset_newtons(
    force_newtons: &[f64],
    start_index: usize,
    end_index: usize,
) -> Option<f64> {
    let end_index = end_index.min(force_newtons.len());
    (start_index < end_index).then(|| mean(&force_newtons[start_index..end_index]))?
}

/// The series with a constant offset taken off every sample.
pub fn remove_offset(force_newtons: &[f64], offset_newtons: f64) -> Option<Vec<f64>> {
    offset_newtons.is_finite().then(|| {
        force_newtons
            .iter()
            .map(|force| force - offset_newtons)
            .collect()
    })
}

/// The two takeoff velocities a linear velocity ramp is built from.
///
/// The vendor policy sets velocity to zero at onset, integrates to takeoff, and adds a ramp
/// closing whatever gap remains against the velocity the same interval's net impulse
/// implies. The two are the same integral, so the gap is the difference between two routes
/// to one quantity rather than a drift.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LinearRampToImpulse {
    /// Accumulated one sample at a time, dividing by mass at each step.
    pub integrated_takeoff_velocity_meters_per_second: f64,
    /// Taken as one compensated impulse over the whole interval, divided by mass once.
    pub impulse_takeoff_velocity_meters_per_second: f64,
}

impl LinearRampToImpulse {
    /// What the ramp adds by takeoff.
    pub fn residual_meters_per_second(&self) -> f64 {
        self.impulse_takeoff_velocity_meters_per_second
            - self.integrated_takeoff_velocity_meters_per_second
    }

    /// What the ramp adds at one sample: nothing at onset, the whole residual at takeoff,
    /// and linear between.
    pub fn correction_at_meters_per_second(
        &self,
        index: usize,
        onset_index: usize,
        takeoff_index: usize,
    ) -> f64 {
        if takeoff_index <= onset_index || index <= onset_index {
            return 0.0;
        }
        let elapsed = (index.min(takeoff_index) - onset_index) as f64;
        let span = (takeoff_index - onset_index) as f64;
        self.residual_meters_per_second() * elapsed / span
    }
}

/// Both routes to the takeoff velocity the interval implies, computed separately so the
/// residual between them is a measurement rather than an identity restated.
pub fn linear_ramp_to_impulse(
    force_newtons: &[f64],
    onset_index: usize,
    takeoff_index: usize,
    system_weight_newtons: f64,
    system_mass_kilograms: f64,
    sample_interval_seconds: f64,
) -> Option<LinearRampToImpulse> {
    let takeoff_index = takeoff_index.min(force_newtons.len().saturating_sub(1));
    if onset_index >= takeoff_index
        || !system_mass_kilograms.is_finite()
        || system_mass_kilograms <= 0.0
        || !system_weight_newtons.is_finite()
        || !sample_interval_seconds.is_finite()
        || sample_interval_seconds <= 0.0
    {
        return None;
    }

    let mut velocity_meters_per_second = 0.0f64;
    for index in onset_index..takeoff_index {
        let acceleration_at =
            (force_newtons[index] - system_weight_newtons) / system_mass_kilograms;
        let acceleration_next =
            (force_newtons[index + 1] - system_weight_newtons) / system_mass_kilograms;
        velocity_meters_per_second +=
            (acceleration_at + acceleration_next) * 0.5 * sample_interval_seconds;
    }

    let mut impulse = CompensatedAccumulator::default();
    for index in onset_index..takeoff_index {
        impulse
            .add((force_newtons[index] + force_newtons[index + 1]) * 0.5 - system_weight_newtons);
    }
    let impulse_newton_seconds = impulse.total() * sample_interval_seconds;

    Some(LinearRampToImpulse {
        integrated_takeoff_velocity_meters_per_second: velocity_meters_per_second,
        impulse_takeoff_velocity_meters_per_second: impulse_newton_seconds / system_mass_kilograms,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const WEIGHT_NEWTONS: f64 = 587.1863;
    const SAMPLE_INTERVAL_SECONDS: f64 = 1.0 / 1200.0;

    fn countermovement() -> Vec<f64> {
        let mut force = vec![WEIGHT_NEWTONS; 1200];
        for (index, sample) in force.iter_mut().enumerate() {
            *sample += ((index % 17) as f64 - 8.0) * 0.4;
        }
        force.extend((0..300).map(|index| WEIGHT_NEWTONS * (1.0 - 0.4 * index as f64 / 300.0)));
        force.extend((0..300).map(|index| WEIGHT_NEWTONS * (0.6 + 1.8 * index as f64 / 300.0)));
        force.extend(std::iter::repeat_n(WEIGHT_NEWTONS * 2.4, 120));
        force.extend(std::iter::repeat_n(0.0, 600));
        force
    }

    /// Onset at the end of quiet standing, takeoff at the end of the push.
    const ONSET_INDEX: usize = 1200;
    const TAKEOFF_INDEX: usize = 1920;

    fn mass_kilograms() -> f64 {
        WEIGHT_NEWTONS / crate::STANDARD_GRAVITY_METERS_PER_SECOND_SQUARED
    }

    #[test]
    fn the_unloaded_window_reports_what_the_plate_read_with_nothing_on_it() {
        let force = countermovement();
        let offset = unloaded_epoch_offset_newtons(&force, TAKEOFF_INDEX, force.len()).unwrap();
        assert!(offset.abs() < 1e-9, "an empty plate read {offset} N");

        let corrected = remove_offset(&force, offset).unwrap();
        assert_eq!(corrected.len(), force.len());
    }

    #[test]
    fn a_window_with_no_samples_in_it_reports_nothing() {
        let force = countermovement();
        assert!(unloaded_epoch_offset_newtons(&force, 900, 900).is_none());
        assert!(unloaded_epoch_offset_newtons(&force, force.len(), force.len() + 10).is_none());
    }

    /// Both quantities the ramp is built from are the same integral, so the ramp it adds is
    /// nothing. The two are computed by different routes here, one accumulating velocity a
    /// sample at a time and one taking a single compensated impulse, so agreement between
    /// them is a measurement rather than the identity written twice.
    #[test]
    fn the_velocity_ramp_adds_nothing_it_did_not_already_have() {
        let force = countermovement();
        let found = linear_ramp_to_impulse(
            &force,
            ONSET_INDEX,
            TAKEOFF_INDEX,
            WEIGHT_NEWTONS,
            mass_kilograms(),
            SAMPLE_INTERVAL_SECONDS,
        )
        .unwrap();

        let residual = found.residual_meters_per_second();
        println!(
            "integrated {} m/s, impulse {} m/s, residual {residual:e} m/s",
            found.integrated_takeoff_velocity_meters_per_second,
            found.impulse_takeoff_velocity_meters_per_second
        );
        assert!(
            found.integrated_takeoff_velocity_meters_per_second.abs() > 1.0,
            "the interval produced no takeoff velocity to compare against"
        );
        assert!(
            residual.abs() < 1e-12,
            "the ramp adds {residual} m/s, which is more than the two routes disagree by"
        );
    }

    #[test]
    fn the_ramp_starts_at_nothing_and_reaches_the_whole_residual_at_takeoff() {
        let force = countermovement();
        let found = linear_ramp_to_impulse(
            &force,
            ONSET_INDEX,
            TAKEOFF_INDEX,
            WEIGHT_NEWTONS,
            mass_kilograms(),
            SAMPLE_INTERVAL_SECONDS,
        )
        .unwrap();

        assert_eq!(
            found.correction_at_meters_per_second(ONSET_INDEX, ONSET_INDEX, TAKEOFF_INDEX),
            0.0
        );
        assert_eq!(
            found.correction_at_meters_per_second(900, ONSET_INDEX, TAKEOFF_INDEX),
            0.0
        );
        assert_eq!(
            found.correction_at_meters_per_second(TAKEOFF_INDEX, ONSET_INDEX, TAKEOFF_INDEX),
            found.residual_meters_per_second()
        );
        let halfway = found.correction_at_meters_per_second(1560, ONSET_INDEX, TAKEOFF_INDEX);
        assert!((halfway - found.residual_meters_per_second() / 2.0).abs() < 1e-18);
    }

    #[test]
    fn an_interval_or_a_mass_no_ramp_can_be_built_on_reports_nothing() {
        let force = countermovement();
        for (onset, takeoff, mass) in [
            (TAKEOFF_INDEX, ONSET_INDEX, mass_kilograms()),
            (ONSET_INDEX, ONSET_INDEX, mass_kilograms()),
            (ONSET_INDEX, TAKEOFF_INDEX, 0.0),
            (ONSET_INDEX, TAKEOFF_INDEX, f64::NAN),
        ] {
            assert!(
                linear_ramp_to_impulse(
                    &force,
                    onset,
                    takeoff,
                    WEIGHT_NEWTONS,
                    mass,
                    SAMPLE_INTERVAL_SECONDS
                )
                .is_none(),
                "onset {onset} takeoff {takeoff} mass {mass} produced a ramp"
            );
        }
    }
}
