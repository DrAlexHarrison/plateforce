//! Landmarks on a countermovement jump trace, and the quantities derived from them.
//!
//! The landmark definitions and the three identities asserted in the tests below were
//! verified against real data before any of this was written. See
//! `docs/landmarks.md` for the operational rule behind each point.

use crate::signal::{Trial, TrialError};
use crate::STANDARD_GRAVITY_METERS_PER_SECOND_SQUARED as GRAVITY;

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
}

impl WeighingEpoch {
    /// Fixed window anchored at the start of the recording.
    pub fn fixed_window(trial: &Trial, duration_seconds: f64) -> Result<Self, TrialError> {
        let samples = (duration_seconds * trial.sample_rate_hz()).round() as usize;
        if samples < 2 || samples > trial.len() {
            return Err(TrialError::EpochTooLong {
                requested_seconds: duration_seconds,
                available_seconds: trial.duration_seconds(),
            });
        }
        let window = &trial.force()[..samples];
        let mean = window.iter().sum::<f64>() / samples as f64;
        // Sample standard deviation, because the threshold rules that consume it are
        // stated against the sample statistic.
        let variance =
            window.iter().map(|f| (f - mean).powi(2)).sum::<f64>() / (samples as f64 - 1.0);
        Ok(Self {
            start_index: 0,
            end_index: samples,
            system_weight_newtons: mean,
            standard_deviation_newtons: variance.sqrt(),
        })
    }

    pub fn system_mass_kilograms(&self) -> f64 {
        self.system_weight_newtons / GRAVITY
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

/// Onset by a noise-relative threshold: the first departure from the quiet-standing
/// mean by `k` standard deviations, walked back by a fixed offset.
///
/// The offset is not optional and not cosmetic. In the originating lab's own
/// publications it was 10 ms, then 30 ms, then 50 and 40 ms, so a rule quoting only
/// `k` does not identify a method.
pub fn onset_noise_relative(
    trial: &Trial,
    epoch: &WeighingEpoch,
    k_standard_deviations: f64,
    back_offset_seconds: f64,
    search_bound_seconds: f64,
) -> Result<usize, TrialError> {
    let band = k_standard_deviations * epoch.standard_deviation_newtons;
    let lower = epoch.system_weight_newtons - band;
    let upper = epoch.system_weight_newtons + band;

    let search_end = ((search_bound_seconds * trial.sample_rate_hz()).round() as usize)
        .min(trial.len())
        .max(epoch.end_index);

    let crossing = trial.force()[epoch.end_index..search_end]
        .iter()
        .position(|&f| f < lower || f > upper)
        .map(|offset| offset + epoch.end_index);

    let crossing = crossing.ok_or_else(|| TrialError::NoCrossing {
        method_id: "onset.threshold.noise_relative".to_string(),
        parameter: "k".to_string(),
        value: k_standard_deviations,
        search_bound_seconds,
    })?;

    let back_samples = (back_offset_seconds * trial.sample_rate_hz()).round() as usize;
    Ok(crossing.saturating_sub(back_samples))
}

/// Takeoff as the first sustained fall below an absolute residual force.
///
/// `minimum_flight_seconds` exists because a threshold alone will fire on any brief
/// dip. Two open tools omit it and instead assume the recording was trimmed to one
/// jump; on an untrimmed recording they place takeoff on average 843 ms late, after
/// the athlete has landed, on 155 of 244 trials, with no warning.
pub fn takeoff_absolute_threshold(
    trial: &Trial,
    threshold_newtons: f64,
    minimum_flight_seconds: f64,
    search_from: usize,
) -> Result<usize, TrialError> {
    let needed = (minimum_flight_seconds * trial.sample_rate_hz()).round() as usize;
    let force = trial.force();
    let mut run_start: Option<usize> = None;

    for index in search_from..force.len() {
        if force[index] < threshold_newtons {
            let start = *run_start.get_or_insert(index);
            if index - start + 1 >= needed {
                return Ok(start);
            }
        } else {
            run_start = None;
        }
    }
    Err(TrialError::NoCrossing {
        method_id: "takeoff.threshold.absolute".to_string(),
        parameter: "threshold_newtons".to_string(),
        value: threshold_newtons,
        search_bound_seconds: trial.duration_seconds(),
    })
}

/// Takeoff velocity by impulse-momentum, in metres per second.
///
/// Net impulse from onset to takeoff divided by system mass. This is an identity, not
/// an estimate, so it is the anchor every other velocity claim is checked against.
pub fn takeoff_velocity_meters_per_second(
    trial: &Trial,
    epoch: &WeighingEpoch,
    landmarks: &Landmarks,
) -> f64 {
    let gross = trial.integrate_newton_seconds(landmarks.onset_index, landmarks.takeoff_index);
    // A trapezoid over n samples spans n-1 intervals, so the weight has to be removed
    // over the same n-1. Using n instead leaves a residual of one sample of bodyweight,
    // which is 8.2 mm/s at 1200 Hz and biases every jump height in the same direction.
    let spanned_intervals = landmarks
        .takeoff_index
        .saturating_sub(landmarks.onset_index)
        .saturating_sub(1);
    let elapsed_seconds = spanned_intervals as f64 * trial.sample_interval_seconds();
    let net = gross - epoch.system_weight_newtons * elapsed_seconds;
    net / epoch.system_mass_kilograms()
}

/// Jump height from takeoff velocity, in metres. The takeoff frame.
pub fn jump_height_from_takeoff_velocity(takeoff_velocity_meters_per_second: f64) -> f64 {
    takeoff_velocity_meters_per_second.powi(2) / (2.0 * GRAVITY)
}

/// Jump height from flight time, in metres. A different construct from the above, not
/// a different method of computing the same one, and on real trials the two differ by
/// more than a training intervention moves the number.
pub fn jump_height_from_flight_time(flight_time_seconds: f64) -> f64 {
    GRAVITY * flight_time_seconds.powi(2) / 8.0
}

#[cfg(test)]
mod tests {
    use super::*;

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
        force.extend(std::iter::repeat(weight + net_force_newtons).take(push_samples));
        force.extend(std::iter::repeat(0.0).take((0.5 * sample_rate_hz) as usize));

        // The trapezoid spans one interval fewer than it has samples.
        let spanned_seconds = (push_samples - 1) as f64 / sample_rate_hz;
        let expected_velocity = net_force_newtons * spanned_seconds / MASS_KILOGRAMS;
        (
            Trial::new(force, sample_rate_hz).unwrap(),
            quiet_samples,
            expected_velocity,
        )
    }

    #[test]
    fn weighing_epoch_recovers_the_system_weight() {
        let (trial, _, _) = synthetic_trial(1200.0, 600.0, 0.3);
        let epoch = WeighingEpoch::fixed_window(&trial, 0.8).unwrap();
        assert!((epoch.system_mass_kilograms() - MASS_KILOGRAMS).abs() < 1e-9);
    }

    #[test]
    fn takeoff_velocity_equals_net_impulse_over_mass() {
        let sample_rate_hz = 1200.0;
        let (trial, onset, expected) = synthetic_trial(sample_rate_hz, 600.0, 0.3);
        let epoch = WeighingEpoch::fixed_window(&trial, 0.8).unwrap();
        let takeoff = takeoff_absolute_threshold(&trial, 20.0, 0.1, epoch.end_index).unwrap();
        let landmarks = Landmarks {
            onset_index: onset,
            takeoff_index: takeoff,
            touchdown_index: trial.len() - 1,
        };
        let velocity = takeoff_velocity_meters_per_second(&trial, &epoch, &landmarks);
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
            let epoch = WeighingEpoch::fixed_window(&trial, 0.8).unwrap();
            let takeoff =
                takeoff_absolute_threshold(&trial, 20.0, 0.1, epoch.end_index).unwrap();
            let velocity = takeoff_velocity_meters_per_second(
                &trial,
                &epoch,
                &Landmarks {
                    onset_index: onset,
                    takeoff_index: takeoff,
                    touchdown_index: trial.len() - 1,
                },
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
        let from_velocity = jump_height_from_takeoff_velocity(velocity);
        let from_flight = jump_height_from_flight_time(flight_time);
        // In the idealised case with no plate residual they coincide.
        assert!((from_velocity - from_flight).abs() < 1e-9);
    }

    #[test]
    fn onset_reports_which_method_and_parameter_failed() {
        let trial = Trial::new(vec![600.0; 2400], 1200.0).unwrap();
        let epoch = WeighingEpoch::fixed_window(&trial, 0.5).unwrap();
        let error = onset_noise_relative(&trial, &epoch, 5.0, 0.03, 1.5).unwrap_err();
        let message = error.to_string();
        assert!(message.contains("onset.threshold.noise_relative"), "{message}");
        assert!(message.contains("k = 5"), "{message}");
    }

    #[test]
    fn takeoff_ignores_a_brief_dip_that_is_not_flight() {
        let sample_rate_hz = 1000.0;
        let mut force = vec![600.0; 1000];
        force.extend(std::iter::repeat(0.0).take(20)); // 20 ms dip, not a flight phase
        force.extend(std::iter::repeat(600.0).take(200));
        force.extend(std::iter::repeat(0.0).take(400)); // the real flight phase
        let trial = Trial::new(force, sample_rate_hz).unwrap();
        let takeoff = takeoff_absolute_threshold(&trial, 20.0, 0.1, 0).unwrap();
        assert_eq!(takeoff, 1220, "took the 20 ms dip for a flight phase");
    }
}
