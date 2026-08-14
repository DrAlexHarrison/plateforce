//! Landmarks on a countermovement jump trace, and the quantities derived from them.
//!
//! See `docs/landmarks.md` for the operational rule behind each point.

use crate::series::{
    centre_of_mass_velocity_meters_per_second, IntegrationAnchor, IntegrationDirection,
    IntegrationSpec, IntegrationStart, QuadratureRule,
};
use crate::signal::{Trial, TrialError};
use crate::statistics::{
    lowest_variance_window, mean_and_standard_deviation, median, DispersionEstimator,
    VarianceAccumulation, WeighingWindowSearch,
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

    /// Weighing window chosen as the quietest stretch of the recording, and the search that
    /// chose it.
    ///
    /// The low force floor keeps the flight phase out of the search, where the plate
    /// is unloaded and almost noiseless and would otherwise win on variance. The
    /// upper bound keeps the search before takeoff for the same reason.
    ///
    /// The search travels out beside the window because the floor removes candidate windows
    /// from consideration, on subject 01's first trial 985 of 4801 of them, and a caller that
    /// receives only the winner cannot say what was taken out of the running.
    pub fn lowest_variance(
        trial: &Trial,
        window_samples: usize,
        search_end_index: usize,
        reject_at_or_below_newtons: Option<f64>,
        accumulation: VarianceAccumulation,
        dispersion: DispersionEstimator,
        centre: CentralTendency,
    ) -> Result<(Self, WeighingWindowSearch), TrialError> {
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
        // The search ranks windows by variance either way; the centre selects only what the
        // chosen window reports as system weight, which is the choice the registry entry
        // declares. The tie fields stay the search's own means, because they describe the
        // ranking rather than the reported weight.
        let centre_newtons = match centre {
            CentralTendency::Mean => found.mean_newtons,
            CentralTendency::Median => median(window).unwrap_or(found.mean_newtons),
        };
        Ok((
            Self {
                start_index: found.start_index,
                end_index: found.start_index + window_samples,
                system_weight_newtons: centre_newtons,
                standard_deviation_newtons: deviation,
                tied_window_count: found.tied_window_count,
                tied_weight_low_newtons: found.tied_weight_low_newtons,
                tied_weight_high_newtons: found.tied_weight_high_newtons,
            },
            found,
        ))
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

/// The four choices the takeoff velocity is read under, as one value a caller can name.
///
/// Public because the number rests on them. `integration.start.trial_start` is deprecated and
/// `integration.start.detected_onset` is recommended, they disagree on any recording carrying
/// quiet stance ahead of the movement, and both force a decision, so a result reporting the
/// velocity without naming these reports three of the rules behind it and hides four.
pub fn takeoff_velocity_integration_spec(landmarks: &Landmarks) -> IntegrationSpec {
    IntegrationSpec {
        quadrature: QuadratureRule::Trapezoid,
        direction: IntegrationDirection::Forward,
        start: IntegrationStart::DetectedOnset {
            index: landmarks.onset_index,
        },
        anchor: IntegrationAnchor::SinglePoint {
            index: landmarks.onset_index,
        },
    }
}

/// Takeoff velocity by impulse-momentum, in metres per second.
///
/// Net impulse from onset to takeoff divided by system mass. This is an identity, not
/// an estimate, so it is the anchor every other velocity claim is checked against.
///
/// Read off the centre-of-mass velocity series rather than integrated again here, under
/// `integration.start.detected_onset`. A caller holding a series built under a different
/// start reads it directly and gets a different, declared, number.
pub fn takeoff_velocity_meters_per_second(
    trial: &Trial,
    epoch: &WeighingEpoch,
    landmarks: &Landmarks,
    gravity_meters_per_second_squared: f64,
) -> f64 {
    let spec = takeoff_velocity_integration_spec(landmarks);
    let series = centre_of_mass_velocity_meters_per_second(
        trial,
        epoch,
        &spec,
        gravity_meters_per_second_squared,
    );
    // Takeoff is the start of the first run below the threshold, so the last sample in
    // contact is the one before it and the interval across it is already flight.
    series
        .at(landmarks.takeoff_index.saturating_sub(1))
        .unwrap_or(0.0)
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
///
/// The one home for the projectile equation. The rule below corrects the number this returns
/// rather than respelling it, so the pair cannot drift apart.
pub fn jump_height_from_flight_time(
    flight_time_seconds: f64,
    gravity_meters_per_second_squared: f64,
) -> f64 {
    gravity_meters_per_second_squared * flight_time_seconds.powi(2) / 8.0
}

/// Jump height from flight time where the centre of mass is not at the same height at landing
/// as at takeoff, in metres.
///
/// `landing_below_takeoff_meters` is how far the centre of mass sits below its takeoff height
/// at the instant of touchdown, so a subject who lands flatter than they took off has a
/// positive value and a shorter jump than the projectile equation reports.
///
/// Goncalves, Baptista, Tufano, Blazevich and Vieira 2024, PeerJ 12:e17704, equations 9 and 10.
/// Their derivation splits the flight into an ascent shortened by the offset and takes the
/// height from the ascent alone, which is published as `(g T / 2 - h / T)^2 / 2g`. Written here
/// as that expression's own factorisation, the projectile height scaled by a posture term,
/// because the two agree to a relative 2e-16 while only this arrangement returns the projectile
/// equation bit for bit at a zero offset. The other one moves the uncorrected height of every
/// trial in the last place, for nothing.
///
/// Uncorrected, the error reaches 59.6 percent of the number on a 0.10 m jump by a 1.98 m
/// subject landing with the ankle flat.
pub fn jump_height_from_flight_time_with_landing_offset(
    flight_time_seconds: f64,
    landing_below_takeoff_meters: f64,
    gravity_meters_per_second_squared: f64,
) -> f64 {
    if flight_time_seconds <= 0.0 {
        return 0.0;
    }
    let uncorrected =
        jump_height_from_flight_time(flight_time_seconds, gravity_meters_per_second_squared);
    let posture_term = 1.0
        - 2.0 * landing_below_takeoff_meters
            / (gravity_meters_per_second_squared * flight_time_seconds.powi(2));
    uncorrected * posture_term.powi(2)
}

/// The heel rise a flight-time height leaves out, in metres.
///
/// The flight-time height measures from the instant of takeoff, at which the ankle is already
/// plantarflexed, so it omits the rise from quiet standing to that instant. Wade, Lichtwark
/// and Farris 2020 stand a constant in for it: the ankle's height above the ground at takeoff,
/// less its height in standing. The sine is 0.88, being sin(61.4 degrees), a single-cohort
/// mean with a 4.8 degree standard deviation that the rule treats as fixed.
///
/// The length the sine multiplies is the **malleolus to toe** distance, not the whole foot.
/// The source's printed formula names the term "Foot Length" and its own text defines it at
/// lines 155 to 156 as "the distance from the medial malleolus to the toes during standing",
/// which are different lengths by the fifth of the foot behind the ankle. Reading the printed
/// name literally puts the constant near 18 cm on a 26 cm foot, against the 10 to 12 cm the
/// same paper reports as the expected range.
///
/// Sole thickness enters because a shoe lifts heel and toe by different amounts, so a barefoot
/// jump states zero here rather than omitting the term.
pub fn heel_rise_constant_meters(
    malleolus_to_toe_length_meters: f64,
    sole_thickness_meters: f64,
    ankle_height_meters: f64,
    takeoff_foot_angle_sine: f64,
) -> f64 {
    takeoff_foot_angle_sine * malleolus_to_toe_length_meters + sole_thickness_meters
        - ankle_height_meters
}

/// The ankle joint's height above the toe in quiet standing, as a length and an angle.
///
/// Goncalves equation 11 builds the ankle-to-toe segment as the hypotenuse of two
/// anthropometric fractions of stature: the ankle sits `ankle_height_fraction` of stature above
/// the ground, and `foot_length_fraction` times `malleolus_fraction` of stature ahead of the
/// toe. At the published fractions the hypotenuse is 0.126 times stature.
///
/// The angle comes back beside the length because the correction needs both and they are the
/// same triangle. Deriving the angle separately from the same three fractions would be a
/// second derivation of one geometry, free to disagree with this one.
pub fn ankle_to_toe_segment(
    stature_meters: f64,
    ankle_height_fraction_of_stature: f64,
    foot_length_fraction_of_stature: f64,
    malleolus_fraction_of_foot_length: f64,
) -> AnkleToToeSegment {
    let rise_meters = ankle_height_fraction_of_stature * stature_meters;
    let reach_meters =
        foot_length_fraction_of_stature * malleolus_fraction_of_foot_length * stature_meters;
    AnkleToToeSegment {
        length_meters: rise_meters.hypot(reach_meters),
        standing_angle_degrees: ankle_to_toe_standing_angle_degrees(
            ankle_height_fraction_of_stature,
            foot_length_fraction_of_stature,
            malleolus_fraction_of_foot_length,
        ),
    }
}

/// How far the ankle-to-toe segment already leans back from horizontal with the foot flat, in
/// degrees.
///
/// Stature cancels out of the ratio, so the lean is a property of the three fractions alone.
/// Separate from the length because a reader who measured the segment on the athlete still
/// needs the lean, and taking it from a stature they did not state would be reading a number
/// out of a value nobody supplied.
pub fn ankle_to_toe_standing_angle_degrees(
    ankle_height_fraction_of_stature: f64,
    foot_length_fraction_of_stature: f64,
    malleolus_fraction_of_foot_length: f64,
) -> f64 {
    ankle_height_fraction_of_stature
        .atan2(foot_length_fraction_of_stature * malleolus_fraction_of_foot_length)
        .to_degrees()
}

/// The ankle-to-toe segment a flight-time correction rotates, from the toe.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AnkleToToeSegment {
    pub length_meters: f64,
    /// How far the segment already leans back from horizontal with the foot flat, which the
    /// measured plantarflexion is added to rather than replacing.
    pub standing_angle_degrees: f64,
}

/// How far the centre of mass sits below its takeoff height at touchdown, from the ankle
/// angles at each instant, in metres.
///
/// Goncalves equation 12. The whole body rides on the ankle-to-toe segment, so rotating that
/// segment from its takeoff angle to its landing angle moves the centre of mass by the
/// difference of the two vertical projections. A subject who takes off plantarflexed and lands
/// flat gives a positive number, which is the case the correction exists for.
pub fn landing_below_takeoff_from_ankle_angles_meters(
    segment: AnkleToToeSegment,
    ankle_angle_at_takeoff_degrees: f64,
    ankle_angle_at_landing_degrees: f64,
) -> f64 {
    let projection = |plantarflexion_degrees: f64| {
        segment.length_meters
            * (plantarflexion_degrees + segment.standing_angle_degrees)
                .to_radians()
                .sin()
    };
    projection(ankle_angle_at_takeoff_degrees) - projection(ankle_angle_at_landing_degrees)
}

/// The downward centre-of-mass velocity a free fall through `drop_height_meters` gives, in
/// metres per second.
///
/// Negative, because the plate's positive direction is up and this is the velocity the athlete
/// arrives with. A drop jump needs it as the lower boundary condition the impulse-momentum
/// integration cannot supply for itself: the integration starts from rest, and on a drop jump
/// the athlete is not at rest when the recording of contact begins.
///
/// A box height is not a drop height. The centre of mass falls less than the box is tall
/// because the athlete steps down rather than dropping rigidly, which is the 0.066 m bias the
/// registry records against the two-plate criterion.
pub fn drop_touchdown_velocity_meters_per_second(
    drop_height_meters: f64,
    gravity_meters_per_second_squared: f64,
) -> f64 {
    if drop_height_meters <= 0.0 {
        return 0.0;
    }
    -(2.0 * gravity_meters_per_second_squared * drop_height_meters).sqrt()
}

pub fn time_to_takeoff_seconds(landmarks: &Landmarks, sample_interval_seconds: f64) -> f64 {
    (landmarks.takeoff_index as f64 - landmarks.onset_index as f64) * sample_interval_seconds
}

/// The interval the athlete was off the plate, from the two samples that bound it.
///
/// Takes the two samples rather than the three-landmark bundle, because the onset is not one
/// of them and a rule that had to assemble the bundle to reach this could not run on a
/// recording whose onset rule found nothing. The arithmetic has one home here either way.
pub fn flight_time_seconds(
    takeoff_index: usize,
    touchdown_index: usize,
    sample_interval_seconds: f64,
) -> f64 {
    (touchdown_index as f64 - takeoff_index as f64) * sample_interval_seconds
}

/// The sample the athlete came back down on, or nothing on a recording that does not carry
/// the return.
///
/// The interval this bounds is the one the athlete was off the plate, so the plate carries
/// nothing at the sample the search opens from and the return is the first run above the
/// threshold that goes on to carry the weighed system weight. Neither condition is decoration.
/// A threshold below the converter's own step is met by every sample a plate can report as
/// nonzero, so one step of dither read as a landing gave a flight of 0.0025 s. A marker
/// dragged onto a loaded stretch of the trace placed the return on takeoff itself and gave a
/// flight of zero, and the interval from such a marker is not a flight at any length: the
/// athlete is standing on the plate through the front of it. The weight separates a plate
/// carrying the athlete from a plate carrying nothing, and it is measured on this trial rather
/// than published anywhere.
///
/// A sample that is not a number ends the run rather than continuing or completing it.
/// Reading an infinity as above the weight would place a landing on the one sample in the
/// recording carrying no measurement.
///
/// A weight that is not a number is a recording whose weighing window lost samples, and it
/// leaves no weight to confirm against. The search then reports the first return the declared
/// rule names, unaugmented, rather than refusing: a gap in the weighing window says nothing
/// about a landing further down the trace, and refusing there took a flight time and a height
/// off a recording whose 2861 N landing is not in doubt.
pub fn return_to_the_plate(
    force_newtons: &[f64],
    takeoff_index: usize,
    threshold_newtons: f64,
    system_weight_newtons: f64,
) -> Option<usize> {
    let at_takeoff = *force_newtons.get(takeoff_index)?;
    if !at_takeoff.is_finite() || at_takeoff > threshold_newtons {
        return None;
    }
    let confirm_against = system_weight_newtons
        .is_finite()
        .then_some(system_weight_newtons);

    let mut run_start: Option<usize> = None;
    for (offset, &force) in force_newtons.get(takeoff_index..)?.iter().enumerate() {
        if !force.is_finite() || force <= threshold_newtons {
            run_start = None;
            continue;
        }
        let start = *run_start.get_or_insert(takeoff_index + offset);
        if confirm_against.is_none_or(|weight| force >= weight) {
            return Some(start);
        }
    }
    None
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

    // Measured on `subject01_trial5`: the plate quantises at 1.398 N, the flight-noise rule
    // re-estimated the threshold at 0.7292 N, and the first step of dither lands three samples
    // after takeoff. Read as a return it gives a flight of 0.0025 s and a height of
    // 0.0000076 m, which a reader averaging the column takes as a jump of nothing.
    const CONVERTER_STEP_NEWTONS: f64 = 1.398;
    const THRESHOLD_BELOW_ONE_STEP_NEWTONS: f64 = 0.7292;
    const SYSTEM_WEIGHT_NEWTONS: f64 = 584.27;

    /// Takeoff, then flight carrying dither, then whatever the caller says came next.
    fn flight_then(dither_at: &[usize], tail: Vec<f64>) -> (Vec<f64>, usize) {
        let takeoff_index = 10;
        let mut force = vec![SYSTEM_WEIGHT_NEWTONS; takeoff_index];
        force.extend(std::iter::repeat_n(0.0, 200));
        for offset in dither_at {
            force[takeoff_index + offset] = CONVERTER_STEP_NEWTONS;
        }
        force.extend(tail);
        (force, takeoff_index)
    }

    /// The landing on `subject01_trial1` reaches system weight seven samples after the
    /// crossing, so a rise of that length is the shape a real return has.
    fn landing() -> Vec<f64> {
        vec![135.6, 271.2, 254.4, 201.3, 254.4, 374.7, 497.7, 629.1, 722.8, 801.0]
    }

    #[test]
    fn a_step_of_dither_after_takeoff_is_not_the_athlete_coming_back_down() {
        let (force, takeoff_index) = flight_then(&[3, 8, 14], Vec::new());
        assert_eq!(
            return_to_the_plate(
                &force,
                takeoff_index,
                THRESHOLD_BELOW_ONE_STEP_NEWTONS,
                SYSTEM_WEIGHT_NEWTONS
            ),
            None,
            "a recording that ends in flight reported a landing"
        );
    }

    #[test]
    fn the_return_is_the_first_sample_of_the_run_that_carries_the_athlete() {
        let (force, takeoff_index) = flight_then(&[3, 8], landing());
        let expected = force
            .iter()
            .rposition(|value| *value == 0.0)
            .expect("the flight is in the trace")
            + 1;
        assert_eq!(
            return_to_the_plate(
                &force,
                takeoff_index,
                THRESHOLD_BELOW_ONE_STEP_NEWTONS,
                SYSTEM_WEIGHT_NEWTONS
            ),
            Some(expected),
            "the return was not placed on the first sample of the rise"
        );
    }

    /// The dither and the landing in one trace, which is what separates skipping an
    /// unconfirmed crossing from refusing the whole recording. A rule that took the first
    /// crossing would answer the dither; one that gave up at the first unconfirmed crossing
    /// would lose a landing that is sitting in the trace.
    #[test]
    fn dither_before_a_real_landing_costs_neither_the_landing_nor_its_place() {
        let (force, takeoff_index) = flight_then(&[3], landing());
        let placed = return_to_the_plate(
            &force,
            takeoff_index,
            THRESHOLD_BELOW_ONE_STEP_NEWTONS,
            SYSTEM_WEIGHT_NEWTONS,
        )
        .expect("the trace carries a landing");
        assert!(
            placed > takeoff_index + 3,
            "the return was placed on the dither at {}",
            takeoff_index + 3
        );
        assert_eq!(force[placed], landing()[0]);
    }

    /// A sample that is not a number ends the run rather than completing it. Both values are
    /// held against it, because rejecting only NaN lets an infinity place a landing on the one
    /// sample in the recording that carries no measurement.
    #[test]
    fn a_sample_that_is_not_a_number_places_no_return() {
        for absent in [f64::NAN, f64::INFINITY] {
            let (force, takeoff_index) = flight_then(&[], vec![absent]);
            assert_eq!(
                return_to_the_plate(&force, takeoff_index, 20.0, SYSTEM_WEIGHT_NEWTONS),
                None,
                "{absent} was read as the athlete returning to the plate"
            );
        }
    }

    /// A recording whose weighing window lost samples still reports the landing it holds.
    ///
    /// The weight is measured over the quiet window and the landing is 700 samples past
    /// takeoff, so a gap in the first says nothing about the second. Confirming against a
    /// weight that is not a number refuses every crossing, and it took the flight time and the
    /// height from it off `subject01_trial1_interrupted`, whose landing peaks at 2861 N: the
    /// two numbers that recording still answered, of eleven, were the two that went.
    #[test]
    fn a_weight_that_is_not_a_number_leaves_the_landing_the_recording_holds() {
        let (force, takeoff_index) = flight_then(&[3], landing());
        let expected = return_to_the_plate(&force, takeoff_index, 20.0, SYSTEM_WEIGHT_NEWTONS)
            .expect("the trace carries a landing");
        for absent in [f64::NAN, f64::INFINITY] {
            assert_eq!(
                return_to_the_plate(&force, takeoff_index, 20.0, absent),
                Some(expected),
                "a weight of {absent} lost the landing rather than the confirmation"
            );
        }
        // And the dither is still not a landing, because the athlete leaving the plate is the
        // other condition and it does not rest on the weight.
        let (dither_only, takeoff_index) = flight_then(&[3, 8, 14], Vec::new());
        assert_eq!(
            return_to_the_plate(
                &dither_only,
                takeoff_index,
                THRESHOLD_BELOW_ONE_STEP_NEWTONS,
                f64::NAN
            ),
            Some(takeoff_index + 3),
            "with no weight to confirm against the declared rule takes the first return, and \
             this names the sample it takes so a change to that is visible here"
        );
    }

    /// The same two values at takeoff itself, where the question is not whether the athlete
    /// came back but whether they ever left. A sample carrying no number cannot show the plate
    /// unloaded, so the interval has no near end and the landing further down the trace is not
    /// a return from anything.
    #[test]
    fn a_takeoff_sample_that_is_not_a_number_bounds_no_flight() {
        for absent in [f64::NAN, f64::INFINITY] {
            let (mut force, takeoff_index) = flight_then(&[], landing());
            force[takeoff_index] = absent;
            assert_eq!(
                return_to_the_plate(&force, takeoff_index, 20.0, SYSTEM_WEIGHT_NEWTONS),
                None,
                "{absent} at takeoff was read as the athlete being off the plate"
            );
        }
    }

    /// A marker dragged onto a loaded stretch of the trace. The plate never unloads after it,
    /// so the athlete never left and there is nothing to return from. Read as a return, the
    /// loaded sample at takeoff gave a flight of zero seconds and a height of zero metres,
    /// which is the sentinel-shaped number this project refuses to write.
    #[test]
    fn a_plate_that_never_unloads_after_takeoff_carries_no_return() {
        let force = vec![SYSTEM_WEIGHT_NEWTONS; 40];
        assert_eq!(return_to_the_plate(&force, 10, 20.0, SYSTEM_WEIGHT_NEWTONS), None);
    }

    /// The same marker on a trace that does carry a real flight and a real landing further on.
    /// The landing is there, and the interval from this marker still is not a flight, because
    /// the athlete is on the plate through the front of it. Refused rather than reported at
    /// whatever length the marker happens to give, which was 1.0167 s and a height of 1.2675 m
    /// on the recording this suite characterises.
    #[test]
    fn a_marker_on_a_loaded_sample_bounds_no_flight_even_where_a_landing_follows() {
        let mut force = vec![SYSTEM_WEIGHT_NEWTONS * 2.0; 20];
        force.extend(std::iter::repeat_n(0.0, 100));
        force.extend(landing());
        assert_eq!(
            return_to_the_plate(&force, 5, 20.0, SYSTEM_WEIGHT_NEWTONS),
            None
        );
        // And the same trace read from a sample where the plate carries nothing answers, so
        // the refusal above is about where the marker sits rather than about this recording.
        assert_eq!(
            return_to_the_plate(&force, 20, 20.0, SYSTEM_WEIGHT_NEWTONS),
            Some(120)
        );
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

    /// The projectile equation is the offset rule at zero offset, and the offset has to move
    /// the answer or the correction is decorative.
    ///
    /// Both halves matter. Agreement alone would pass against a rule that ignores its offset,
    /// and a moved number alone would pass against a rule that no longer reduces to the
    /// equation nine studies published.
    ///
    /// Exact equality rather than a tolerance, because the correction is written as a factor on
    /// the projectile height for the sake of this: a rearrangement that agrees only to a
    /// tolerance moves the uncorrected height of every trial in its last place.
    #[test]
    fn a_flight_time_height_with_no_landing_offset_is_the_projectile_equation() {
        for flight_time in [0.35, 0.5, 0.676, 0.8] {
            let plain = jump_height_from_flight_time(flight_time, GRAVITY);
            let offset_free =
                jump_height_from_flight_time_with_landing_offset(flight_time, 0.0, GRAVITY);
            assert_eq!(
                plain, offset_free,
                "at {flight_time} s the two arrangements gave {plain} and {offset_free}"
            );
        }

        // Landing below takeoff lengthens the fall, so the uncorrected number is the larger
        // one. 0.04 m is the middle of the ankle-position range the source simulates.
        let uncorrected = jump_height_from_flight_time(0.4, GRAVITY);
        let corrected = jump_height_from_flight_time_with_landing_offset(0.4, 0.04, GRAVITY);
        println!(
            "0.4 s flight: {uncorrected:.4} m uncorrected, {corrected:.4} m at a 0.04 m offset"
        );
        assert!(
            corrected < uncorrected - 0.01,
            "a 0.04 m landing offset moved a {uncorrected:.4} m jump to {corrected:.4} m"
        );
    }

    /// The published length equation returns the published number.
    ///
    /// `l_at = sqrt((0.039H)^2 + (0.152 x 0.787 H)^2) = 0.126H` is Goncalves equation 11, and
    /// the entry publishes two of the three fractions. Checking the constant rather than the
    /// arithmetic is what catches a fraction going missing: drop the ankle height and the
    /// answer is 0.120H, which still looks like a length.
    #[test]
    fn the_ankle_to_toe_segment_matches_the_published_fraction_of_stature() {
        let segment = ankle_to_toe_segment(1.71, 0.039, 0.152, 0.787);
        let fraction = segment.length_meters / 1.71;
        println!(
            "at 1.71 m stature the segment is {:.4} m, {fraction:.4} of stature, leaning {:.2} degrees",
            segment.length_meters, segment.standing_angle_degrees
        );
        assert!(
            (fraction - 0.126).abs() < 0.0005,
            "the segment came to {fraction:.4} of stature against the published 0.126"
        );
        assert!(
            (segment.standing_angle_degrees - 18.06).abs() < 0.05,
            "the standing lean came to {:.2} degrees",
            segment.standing_angle_degrees
        );
    }

    /// Taking off plantarflexed and landing flat puts the centre of mass below where it left,
    /// and taking off and landing alike puts it nowhere.
    #[test]
    fn an_unchanged_ankle_moves_the_centre_of_mass_nowhere_and_a_changed_one_moves_it_down() {
        let segment = ankle_to_toe_segment(1.71, 0.039, 0.152, 0.787);
        let unchanged = landing_below_takeoff_from_ankle_angles_meters(segment, 40.0, 40.0);
        assert!(
            unchanged.abs() < 1e-12,
            "an unchanged ankle moved the centre of mass {unchanged} m"
        );

        let flat_landing = landing_below_takeoff_from_ankle_angles_meters(segment, 40.0, 0.0);
        println!("40 degrees of plantarflexion lost at landing: {flat_landing:.4} m");
        assert!(
            flat_landing > 0.0,
            "landing flatter than takeoff put the centre of mass {flat_landing} m above it"
        );
        // The reverse posture reverses the sign rather than taking a magnitude, because a
        // subject who lands more plantarflexed than they took off has a longer jump than the
        // projectile equation reports, not a shorter one.
        let plantarflexed_landing =
            landing_below_takeoff_from_ankle_angles_meters(segment, 0.0, 40.0);
        assert!(
            (plantarflexed_landing + flat_landing).abs() < 1e-12,
            "the two postures gave {flat_landing} and {plantarflexed_landing}"
        );
    }

    /// The heel-rise constant lands in the range the source reports, and sole thickness is a
    /// term rather than a rounding.
    ///
    /// The range is the assertion that earns its keep. The arithmetic is three operations and
    /// checking it against itself would pass on the length the source's printed formula names
    /// rather than the one its text defines, which puts the constant near 18 cm against the
    /// 10 to 12 cm the same paper calls the expected range.
    #[test]
    fn the_heel_rise_constant_lands_in_the_range_its_source_reports() {
        // A 1.71 m subject: 0.26 m foot, so 0.787 of it is 0.205 m of malleolus to toe, a
        // 0.02 m sole and a 0.067 m ankle height.
        let malleolus_to_toe = 0.787 * 0.26;
        let constant = heel_rise_constant_meters(malleolus_to_toe, 0.02, 0.067, 0.88);
        println!("heel rise from a {malleolus_to_toe:.4} m malleolus-to-toe: {constant:.4} m");
        assert!(
            (0.09..=0.14).contains(&constant),
            "the constant came to {constant:.4} m, outside the 0.10 to 0.12 m the source reports"
        );

        // Reading the printed name literally is the error this range catches, and it is a
        // whole heel rise wide.
        let whole_foot = heel_rise_constant_meters(0.26, 0.02, 0.067, 0.88);
        assert!(
            whole_foot > 0.17,
            "the whole-foot reading came to {whole_foot:.4} m, so the range above proves nothing"
        );

        // Barefoot is the same jump with the sole term at zero, which the rule states rather
        // than omits.
        let barefoot = heel_rise_constant_meters(malleolus_to_toe, 0.0, 0.067, 0.88);
        assert!((constant - barefoot - 0.02).abs() < 1e-12, "{barefoot}");
    }

    /// A drop from a stated height arrives moving downward, and a drop from nothing arrives
    /// at rest.
    #[test]
    fn a_drop_height_gives_a_downward_touchdown_velocity() {
        let from_thirty_centimetres = drop_touchdown_velocity_meters_per_second(0.30, GRAVITY);
        println!("a 0.30 m drop arrives at {from_thirty_centimetres:.4} m/s");
        assert!(
            from_thirty_centimetres < 0.0,
            "the athlete arrived travelling upward at {from_thirty_centimetres} m/s"
        );
        assert!(
            (from_thirty_centimetres + (2.0f64 * GRAVITY * 0.30).sqrt()).abs() < 1e-12,
            "{from_thirty_centimetres}"
        );
        assert_eq!(drop_touchdown_velocity_meters_per_second(0.0, GRAVITY), 0.0);
    }
}
