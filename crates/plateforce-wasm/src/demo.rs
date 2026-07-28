//! A synthetic countermovement jump, for the interface to open with.
//!
//! Synthetic on purpose. The corpus this project was built from is re-identifiable, so
//! no recorded trial ships with the software. The shape is drawn to the landmark
//! definitions in `docs/landmarks.md` and the propulsion amplitude is then solved so
//! that net impulse over system mass equals the stated takeoff velocity, which keeps the
//! demonstration internally consistent rather than merely plausible.

use plateforce_core::takeoff::{takeoff_first_sustained_run, ResidualComparison};
use plateforce_core::trial::{takeoff_velocity_meters_per_second, CentralTendency};
use plateforce_core::DispersionEstimator;
use plateforce_core::Trial;
use plateforce_core::STANDARD_GRAVITY_METERS_PER_SECOND_SQUARED as GRAVITY;
use plateforce_core::{Landmarks, WeighingEpoch};

const SAMPLE_RATE_HZ: f64 = 1200.0;
const DURATION_SECONDS: f64 = 5.0;
const SYSTEM_MASS_KILOGRAMS: f64 = 60.0;
const TARGET_TAKEOFF_VELOCITY_METERS_PER_SECOND: f64 = 2.83;

const ONSET_SECONDS: f64 = 3.44;
const TROUGH_SECONDS: f64 = 3.58;
const PEAK_SECONDS: f64 = 4.07;
const TAKEOFF_SECONDS: f64 = 4.18;
const UNWEIGHTING_DEPTH_NEWTONS: f64 = 335.0;
const LANDING_RISE_SECONDS: f64 = 0.05;
const LANDING_PEAK_NEWTONS: f64 = 2600.0;

/// Deterministic plate noise. A seeded generator rather than a random one, so two people
/// looking at the demonstration are looking at the same trace.
struct Noise(u64);

impl Noise {
    fn next_unit(&mut self) -> f64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((self.0 >> 33) as f64 / (1u64 << 31) as f64) - 1.0
    }
}

fn half_cosine(from: f64, to: f64, position: f64) -> f64 {
    from + (to - from) * (1.0 - (position * std::f64::consts::PI).cos()) / 2.0
}

fn build(propulsion_amplitude_newtons: f64, flight_seconds: f64) -> Vec<f64> {
    let system_weight_newtons = SYSTEM_MASS_KILOGRAMS * GRAVITY;
    let sample_count = (DURATION_SECONDS * SAMPLE_RATE_HZ) as usize;
    let landing_seconds = TAKEOFF_SECONDS + flight_seconds;
    let mut noise = Noise(0x5eed_0fce_1234_abcd);

    (0..sample_count)
        .map(|index| {
            let seconds = index as f64 / SAMPLE_RATE_HZ;
            let force = if seconds < ONSET_SECONDS {
                system_weight_newtons
            } else if seconds < TROUGH_SECONDS {
                half_cosine(
                    system_weight_newtons,
                    system_weight_newtons - UNWEIGHTING_DEPTH_NEWTONS,
                    (seconds - ONSET_SECONDS) / (TROUGH_SECONDS - ONSET_SECONDS),
                )
            } else if seconds < PEAK_SECONDS {
                half_cosine(
                    system_weight_newtons - UNWEIGHTING_DEPTH_NEWTONS,
                    system_weight_newtons + propulsion_amplitude_newtons,
                    (seconds - TROUGH_SECONDS) / (PEAK_SECONDS - TROUGH_SECONDS),
                )
            } else if seconds < TAKEOFF_SECONDS {
                half_cosine(
                    system_weight_newtons + propulsion_amplitude_newtons,
                    0.0,
                    (seconds - PEAK_SECONDS) / (TAKEOFF_SECONDS - PEAK_SECONDS),
                )
            } else if seconds < landing_seconds {
                0.0
            } else if seconds < landing_seconds + LANDING_RISE_SECONDS {
                half_cosine(
                    0.0,
                    LANDING_PEAK_NEWTONS,
                    (seconds - landing_seconds) / LANDING_RISE_SECONDS,
                )
            } else {
                let decay = (seconds - landing_seconds - LANDING_RISE_SECONDS) / 0.35;
                system_weight_newtons
                    + (LANDING_PEAK_NEWTONS - system_weight_newtons) * (-decay * 3.0).exp()
            };

            // Flight is not perfectly quiet on a real plate, and standing is not perfectly
            // still. Both matter: the first defeats a zero-exact takeoff test, the second
            // is what the noise-relative onset rule measures itself against.
            let amplitude = if (TAKEOFF_SECONDS..landing_seconds).contains(&seconds) {
                0.25
            } else {
                2.0
            };
            force + noise.next_unit() * amplitude
        })
        .collect()
}

/// Solve the propulsion amplitude so the velocity the pipeline reports is the velocity
/// the trace was drawn to have.
///
/// The bisection runs the same weighing epoch, the same takeoff rule and the same
/// impulse-momentum call a user gets, rather than integrating between the nominal times.
/// Calibrating against nominal times leaves the detected takeoff a few samples early and
/// the demonstration then disagrees with itself by about 2.5 percent.
pub fn synthetic_countermovement_jump() -> Trial {
    let flight_seconds = 2.0 * TARGET_TAKEOFF_VELOCITY_METERS_PER_SECOND / GRAVITY;
    let onset_index = (ONSET_SECONDS * SAMPLE_RATE_HZ) as usize;

    let velocity_for = |amplitude: f64| {
        let trial = Trial::new(build(amplitude, flight_seconds), SAMPLE_RATE_HZ).unwrap();
        let epoch = WeighingEpoch::fixed_window(
            &trial,
            1.0,
            CentralTendency::Mean,
            DispersionEstimator::Sample,
        )
        .unwrap();
        let takeoff_index = takeoff_first_sustained_run(
            trial.force(),
            10.0,
            (0.1 * SAMPLE_RATE_HZ) as usize,
            ResidualComparison::SignedValue,
            epoch.end_index,
            SAMPLE_RATE_HZ,
        )
        .unwrap_or(trial.len() - 1);
        takeoff_velocity_meters_per_second(
            &trial,
            &epoch,
            &Landmarks {
                onset_index,
                takeoff_index,
                touchdown_index: trial.len() - 1,
            },
            GRAVITY,
        )
    };

    let mut low = 0.0;
    let mut high = 4000.0;
    for _ in 0..60 {
        let middle = (low + high) / 2.0;
        if velocity_for(middle) < TARGET_TAKEOFF_VELOCITY_METERS_PER_SECOND {
            low = middle;
        } else {
            high = middle;
        }
    }

    Trial::new(build((low + high) / 2.0, flight_seconds), SAMPLE_RATE_HZ).unwrap()
}

#[cfg(test)]
mod tests {
    use plateforce_core::jump_height_from_takeoff_velocity;
    use plateforce_core::onset::{
        onset_noise_relative, BandSides, CrossingSearch, CrossingSelection, DegenerateBandPolicy,
    };

    use super::*;

    #[test]
    fn the_demonstration_trial_has_the_shape_of_the_measured_corpus() {
        let trial = synthetic_countermovement_jump();
        assert_eq!(trial.len(), 6000);
        assert!((trial.sample_rate_hz() - 1200.0).abs() < 1e-9);
    }

    #[test]
    fn its_impulse_and_its_stated_takeoff_velocity_agree() {
        let trial = synthetic_countermovement_jump();
        let epoch = WeighingEpoch::fixed_window(
            &trial,
            1.0,
            CentralTendency::Mean,
            DispersionEstimator::Sample,
        )
        .unwrap();
        let takeoff = takeoff_first_sustained_run(
            trial.force(),
            10.0,
            (0.1 * SAMPLE_RATE_HZ) as usize,
            ResidualComparison::SignedValue,
            epoch.end_index,
            SAMPLE_RATE_HZ,
        )
        .unwrap();
        let onset = (ONSET_SECONDS * SAMPLE_RATE_HZ) as usize;
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
            (velocity - TARGET_TAKEOFF_VELOCITY_METERS_PER_SECOND).abs() < 0.02,
            "takeoff velocity solved to {velocity}"
        );
        let height = jump_height_from_takeoff_velocity(velocity, GRAVITY);
        assert!(
            (0.30..0.50).contains(&height),
            "jump height {height} m is not a jump"
        );
    }

    #[test]
    fn the_published_onset_rule_finds_an_onset_on_it() {
        let trial = synthetic_countermovement_jump();
        let epoch = WeighingEpoch::fixed_window(
            &trial,
            1.0,
            CentralTendency::Mean,
            DispersionEstimator::Sample,
        )
        .unwrap();
        let onset = onset_noise_relative(
            trial.force(),
            epoch.system_weight_newtons,
            epoch.standard_deviation_newtons,
            5.0,
            BandSides::BothSides,
            DegenerateBandPolicy::Refuse,
            &CrossingSearch {
                start_index: epoch.end_index,
                end_index: trial.len(),
                persistence_samples: 1,
                selection: CrossingSelection::First,
            },
            SAMPLE_RATE_HZ,
        );
        assert!(onset.is_ok(), "{onset:?}");
    }
}
