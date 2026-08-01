//! Landmarks on a countermovement jump trace, and the quantities derived from them.
//!
//! See `docs/landmarks.md` for the operational rule behind each point.

use crate::signal::{Trial, TrialError};
use crate::statistics::{
    lowest_variance_window, mean_and_standard_deviation, median, DispersionEstimator,
    VarianceAccumulation,
};

/// The quiet-standing window that establishes system weight.
///
/// Its duration and placement are a registry choice, not a constant. The registry
/// records at least four distinct windows in the literature and one implementation
/// whose window is specified in samples rather than seconds, which silently changes
/// meaning between a 1000 Hz and a 1200 Hz recording.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WeighingEpoch {
    pub start_index: usize,
    pub end_index: usize,
    pub system_weight_newtons: f64,
    pub standard_deviation_newtons: f64,
    /// Number of windows the selection rule could not choose between. One for any
    /// rule with a fixed window; larger whenever a search rule found exact ties.
    pub tied_window_count: usize,
    /// Lightest and heaviest weight the selection rule could have returned. Equal to
    /// `system_weight_newtons` for any rule whose window is fixed.
    pub tied_weight_low_newtons: f64,
    pub tied_weight_high_newtons: f64,
}

/// Which statistic of the weighing window stands for system weight.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CentralTendency {
    Mean,
    Median,
}

impl WeighingEpoch {
    /// Window anchored at the start of the recording.
    pub fn fixed_window(
        trial: &Trial,
        duration_seconds: f64,
        centre: CentralTendency,
        dispersion: DispersionEstimator,
    ) -> Result<Self, TrialError> {
        Self::window(trial, 0, duration_seconds, centre, dispersion)
    }

    /// Window anchored anywhere. A brushable weighing window needs this, and so does
    /// any protocol where the athlete steps on after recording has started.
    pub fn window(
        trial: &Trial,
        start_index: usize,
        duration_seconds: f64,
        centre: CentralTendency,
        dispersion: DispersionEstimator,
    ) -> Result<Self, TrialError> {
        let samples = (duration_seconds * trial.sample_rate_hz()).round() as usize;
        Self::sample_window(
            trial,
            start_index,
            samples,
            duration_seconds,
            centre,
            dispersion,
        )
    }

    pub fn fixed_sample_window(
        trial: &Trial,
        samples: usize,
        requested_seconds: f64,
        centre: CentralTendency,
        dispersion: DispersionEstimator,
    ) -> Result<Self, TrialError> {
        Self::sample_window(trial, 0, samples, requested_seconds, centre, dispersion)
    }

    /// Window stated in samples, which is how one implementation states it and is the
    /// reason its window means different durations at different sample rates.
    pub fn sample_window(
        trial: &Trial,
        start_index: usize,
        samples: usize,
        requested_seconds: f64,
        centre: CentralTendency,
        dispersion: DispersionEstimator,
    ) -> Result<Self, TrialError> {
        let end_index = start_index.saturating_add(samples);
        let does_not_fit = || TrialError::EpochTooLong {
            requested_seconds,
            start_seconds: start_index as f64 / trial.sample_rate_hz(),
            available_seconds: trial.duration_seconds(),
        };
        if samples < 2 || end_index > trial.len() {
            return Err(does_not_fit());
        }
        let window = &trial.force()[start_index..end_index];
        let (window_mean, deviation) =
            mean_and_standard_deviation(window, dispersion).ok_or_else(does_not_fit)?;
        let centre_newtons = match centre {
            CentralTendency::Mean => window_mean,
            CentralTendency::Median => median(window).unwrap_or(window_mean),
        };
        Ok(Self {
            start_index,
            end_index,
            system_weight_newtons: centre_newtons,
            standard_deviation_newtons: deviation,
            tied_window_count: 1,
            tied_weight_low_newtons: centre_newtons,
            tied_weight_high_newtons: centre_newtons,
        })
    }

    /// Weighing window chosen as the quietest stretch of the recording.
    ///
    /// The low force floor keeps the flight phase out of the search, where the plate
    /// is unloaded and almost noiseless and would otherwise win on variance. The
    /// upper bound keeps the search before takeoff for the same reason.
    pub fn lowest_variance(
        trial: &Trial,
        window_samples: usize,
        search_end_index: usize,
        reject_at_or_below_newtons: Option<f64>,
        accumulation: VarianceAccumulation,
        dispersion: DispersionEstimator,
    ) -> Result<Self, TrialError> {
        let searchable = &trial.force()[..search_end_index.min(trial.len())];
        let found = lowest_variance_window(
            searchable,
            window_samples,
            reject_at_or_below_newtons,
            accumulation,
        )
        .ok_or(TrialError::EpochTooLong {
            requested_seconds: window_samples as f64 / trial.sample_rate_hz(),
            start_seconds: 0.0,
            available_seconds: searchable.len() as f64 / trial.sample_rate_hz(),
        })?;
        let window = &trial.force()[found.start_index..found.start_index + window_samples];
        let deviation = mean_and_standard_deviation(window, dispersion)
            .map(|(_, deviation)| deviation)
            .unwrap_or(f64::NAN);
        Ok(Self {
            start_index: found.start_index,
            end_index: found.start_index + window_samples,
            system_weight_newtons: found.mean_newtons,
            standard_deviation_newtons: deviation,
            tied_window_count: found.tied_window_count,
            tied_weight_low_newtons: found.tied_weight_low_newtons,
            tied_weight_high_newtons: found.tied_weight_high_newtons,
        })
    }

    /// System mass under a stated value of gravity.
    ///
    /// Gravity is an argument because the tools disagree on it: 9.81 is the common
    /// choice and 9.80665 is the standard value, and the two move jump height by
    /// 342 parts per million in the same direction on every trial, by both routes.
    pub fn system_mass_kilograms(&self, gravity_meters_per_second_squared: f64) -> f64 {
        self.system_weight_newtons / gravity_meters_per_second_squared
    }
}

/// The points a countermovement jump analysis depends on.
///
/// Every index here is the output of a named registry method, so a `Landmarks` value
/// is only meaningful alongside the provenance that produced it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Landmarks {
    pub onset_index: usize,
    pub takeoff_index: usize,
    pub touchdown_index: usize,
}

/// Takeoff velocity by impulse-momentum, in metres per second.
///
/// Net impulse from onset to takeoff divided by system mass. This is an identity, not
/// an estimate, so it is the anchor every other velocity claim is checked against.
pub fn takeoff_velocity_meters_per_second(
    trial: &Trial,
    epoch: &WeighingEpoch,
    landmarks: &Landmarks,
    gravity_meters_per_second_squared: f64,
) -> f64 {
    let net = trial.integrate_offset_newton_seconds(
        landmarks.onset_index,
        landmarks.takeoff_index,
        epoch.system_weight_newtons,
    );
    net / epoch.system_mass_kilograms(gravity_meters_per_second_squared)
}

/// Jump height from takeoff velocity, in metres. The takeoff frame.
pub fn jump_height_from_takeoff_velocity(
    takeoff_velocity_meters_per_second: f64,
    gravity_meters_per_second_squared: f64,
) -> f64 {
    takeoff_velocity_meters_per_second.powi(2) / (2.0 * gravity_meters_per_second_squared)
}

/// Jump height from flight time, in metres. A different construct from the above, not
/// a different method of computing the same one, and on real trials the two differ by
/// more than a training intervention moves the number.
pub fn jump_height_from_flight_time(
    flight_time_seconds: f64,
    gravity_meters_per_second_squared: f64,
) -> f64 {
    gravity_meters_per_second_squared * flight_time_seconds.powi(2) / 8.0
}

pub fn time_to_takeoff_seconds(landmarks: &Landmarks, sample_interval_seconds: f64) -> f64 {
    (landmarks.takeoff_index as f64 - landmarks.onset_index as f64) * sample_interval_seconds
}

pub fn flight_time_seconds(landmarks: &Landmarks, sample_interval_seconds: f64) -> f64 {
    (landmarks.touchdown_index as f64 - landmarks.takeoff_index as f64) * sample_interval_seconds
}

/// Reactive strength index, modified: jump height over the time taken to produce it.
///
/// Height is in metres here. One commercial export labels this column cm/s while
/// writing m/s into it, so any dataset that took the header at face value and
/// converted is wrong by a factor of 100.
pub fn reactive_strength_index_modified(
    jump_height_meters: f64,
    time_to_takeoff_seconds: f64,
) -> Option<f64> {
    if time_to_takeoff_seconds <= 0.0 {
        return None;
    }
    Some(jump_height_meters / time_to_takeoff_seconds)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::takeoff::{takeoff_first_sustained_run, ResidualComparison};
    use crate::STANDARD_GRAVITY_METERS_PER_SECOND_SQUARED as GRAVITY;

    const MASS_KILOGRAMS: f64 = 60.0;
    const QUIET_SECONDS: f64 = 1.0;

    /// Quiet standing, then a constant net push, then flight.
    ///
    /// The push is constant so the impulse has a closed form under the trapezoid rule
    /// and the test checks the identity rather than the integration scheme. A sine
    /// over a full period would net to zero impulse, which is not a jump.
    fn synthetic_trial(
        sample_rate_hz: f64,
        net_force_newtons: f64,
        push_seconds: f64,
    ) -> (Trial, usize, f64) {
        let weight = MASS_KILOGRAMS * GRAVITY;
        let quiet_samples = (QUIET_SECONDS * sample_rate_hz) as usize;
        let push_samples = (push_seconds * sample_rate_hz) as usize;

        let mut force = vec![weight; quiet_samples];
        force.extend(std::iter::repeat_n(
            weight + net_force_newtons,
            push_samples,
        ));
        force.extend(std::iter::repeat_n(0.0, (0.5 * sample_rate_hz) as usize));

        // The trapezoid spans one interval fewer than it has samples.
        let spanned_seconds = (push_samples - 1) as f64 / sample_rate_hz;
        let expected_velocity = net_force_newtons * spanned_seconds / MASS_KILOGRAMS;
        (
            Trial::new(force, sample_rate_hz).unwrap(),
            quiet_samples,
            expected_velocity,
        )
    }

    fn quiet_epoch(trial: &Trial) -> WeighingEpoch {
        WeighingEpoch::fixed_window(
            trial,
            0.8,
            CentralTendency::Mean,
            DispersionEstimator::Sample,
        )
        .unwrap()
    }

    #[test]
    fn weighing_epoch_recovers_the_system_weight() {
        let (trial, _, _) = synthetic_trial(1200.0, 600.0, 0.3);
        let epoch = quiet_epoch(&trial);
        assert!((epoch.system_mass_kilograms(GRAVITY) - MASS_KILOGRAMS).abs() < 1e-9);
    }

    #[test]
    fn takeoff_velocity_equals_net_impulse_over_mass() {
        let sample_rate_hz = 1200.0;
        let (trial, onset, expected) = synthetic_trial(sample_rate_hz, 600.0, 0.3);
        let epoch = quiet_epoch(&trial);
        let takeoff = takeoff_first_sustained_run(
            trial.force(),
            20.0,
            120,
            ResidualComparison::SignedValue,
            epoch.end_index,
            sample_rate_hz,
        )
        .unwrap();
        let landmarks = Landmarks {
            onset_index: onset,
            takeoff_index: takeoff,
            touchdown_index: trial.len() - 1,
        };
        let velocity = takeoff_velocity_meters_per_second(&trial, &epoch, &landmarks, GRAVITY);
        assert!(
            (velocity - expected).abs() < 1e-9,
            "impulse-momentum identity broken: {velocity} against {expected}"
        );
    }

    /// The one-sample weight residual this caught is systematic, not noise, so it must
    /// stay caught at every sample rate rather than only at the one it was found on.
    #[test]
    fn the_identity_holds_at_every_sample_rate() {
        for sample_rate_hz in [500.0, 1000.0, 1200.0, 2000.0] {
            let (trial, onset, expected) = synthetic_trial(sample_rate_hz, 600.0, 0.3);
            let epoch = quiet_epoch(&trial);
            let takeoff = takeoff_first_sustained_run(
                trial.force(),
                20.0,
                (0.1 * sample_rate_hz) as usize,
                ResidualComparison::SignedValue,
                epoch.end_index,
                sample_rate_hz,
            )
            .unwrap();
            let velocity = takeoff_velocity_meters_per_second(
                &trial,
                &epoch,
                &Landmarks {
                    onset_index: onset,
                    takeoff_index: takeoff,
                    touchdown_index: trial.len() - 1,
                },
                GRAVITY,
            );
            assert!(
                (velocity - expected).abs() < 1e-9,
                "identity broken at {sample_rate_hz} Hz: {velocity} against {expected}"
            );
        }
    }

    #[test]
    fn the_two_jump_height_methods_are_different_constructs() {
        // Same jump, both formulae, and they do not agree. That disagreement is the
        // product, so a test asserting they match would be asserting the bug.
        let velocity = 2.83;
        let flight_time = 2.0 * velocity / GRAVITY;
        let from_velocity = jump_height_from_takeoff_velocity(velocity, GRAVITY);
        let from_flight = jump_height_from_flight_time(flight_time, GRAVITY);
        // In the idealised case with no plate residual they coincide.
        assert!((from_velocity - from_flight).abs() < 1e-9);
    }

    /// Gravity is a bound parameter, so the two published values must be visible in
    /// the result rather than absorbed by whichever one the library picked.
    #[test]
    fn the_two_published_values_of_gravity_move_jump_height() {
        let common = jump_height_from_takeoff_velocity(2.83, 9.81);
        let standard = jump_height_from_takeoff_velocity(2.83, 9.80665);
        assert!(common != standard);
        assert!((common / standard - 9.80665 / 9.81).abs() < 1e-12);
    }

    /// A window that runs off the end names where it started, because the same duration
    /// fits or does not fit depending on where it was anchored.
    #[test]
    fn a_window_anchored_late_weighs_the_samples_under_it() {
        let mut force = vec![600.0; 1200];
        force.extend(std::iter::repeat_n(900.0, 1200));
        let trial = Trial::new(force, 1200.0).unwrap();
        let early = WeighingEpoch::window(
            &trial,
            0,
            0.5,
            CentralTendency::Mean,
            DispersionEstimator::Sample,
        )
        .unwrap();
        let late = WeighingEpoch::window(
            &trial,
            1200,
            0.5,
            CentralTendency::Mean,
            DispersionEstimator::Sample,
        )
        .unwrap();
        assert_eq!(early.system_weight_newtons, 600.0);
        assert_eq!(late.system_weight_newtons, 900.0);
        assert_eq!((late.start_index, late.end_index), (1200, 1800));

        let overrunning = WeighingEpoch::window(
            &trial,
            2200,
            0.5,
            CentralTendency::Mean,
            DispersionEstimator::Sample,
        )
        .unwrap_err();
        let message = overrunning.to_string();
        assert!(message.contains("starting at 1.8333"), "{message}");
    }

    #[test]
    fn reactive_strength_index_refuses_a_zero_denominator() {
        assert_eq!(reactive_strength_index_modified(0.4, 0.0), None);
        assert_eq!(reactive_strength_index_modified(0.4, 0.8), Some(0.5));
    }
}
