//! Mechanical power and work, over intervals a phase model declared.
//!
//! Power here is a product of force and velocity, and neither term is differentiated: the
//! force is measured and the velocity is integrated from it. That matters for reading the
//! result, because in the pipelines this competes with one of the two terms comes from
//! differentiating a displacement, and a differentiated term dominates the noise in the
//! product.
//!
//! Work is one integral. Force through a displacement and power through a time are the same
//! quantity in continuous form, so shipping an integral for each would be two answers to one
//! question; the two registry rules that name them select on whether a measured displacement
//! signal exists, not on what to compute.

use crate::phases::{cumulative_trapezoid, PhaseModelBoundaries};
use crate::series::VelocitySeries;

#[derive(Debug, thiserror::Error)]
pub enum PowerError {
    #[error("the declared phase runs {first_index}..{last_index} and the trace holds {sample_count} samples")]
    PhaseOutsideTrace {
        first_index: usize,
        last_index: usize,
        sample_count: usize,
    },
    #[error("the declared phase spans {sample_count} samples and an interval needs at least two")]
    PhaseTooShort { sample_count: usize },
    #[error(
        "force holds {force_samples} samples and the velocity series holds {velocity_samples}"
    )]
    SeriesLengthMismatch {
        force_samples: usize,
        velocity_samples: usize,
    },
    #[error(
        "a phase model declared {boundary_count} boundaries and segment {segment} was asked for"
    )]
    SegmentOutsideModel {
        segment: usize,
        boundary_count: usize,
    },
}

/// Which force stands in the product.
///
/// The two differ by exactly one system weight at every instant, so a power computed
/// against the wrong one is wrong by a fixed and invisible amount.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForceTerm {
    /// The force the plate measured.
    GroundReaction,
    /// Measured force less system weight, which is the force that accelerated the centre of
    /// mass rather than the force that also held it up.
    NetOfSystemWeight,
}

/// Which direction of motion counts as positive power.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerSignConvention {
    UpwardPositive,
    DownwardPositive,
}

/// Instantaneous power in watts, and the two choices that produced it.
///
/// A consumer cannot receive the samples without receiving what produced them.
#[derive(Debug, Clone, PartialEq)]
pub struct PowerSeries {
    watts: Vec<f64>,
    force_term: ForceTerm,
    sign_convention: PowerSignConvention,
}

impl PowerSeries {
    pub fn watts(&self) -> &[f64] {
        &self.watts
    }

    pub fn force_term(&self) -> ForceTerm {
        self.force_term
    }

    pub fn sign_convention(&self) -> PowerSignConvention {
        self.sign_convention
    }

    pub fn len(&self) -> usize {
        self.watts.len()
    }

    pub fn is_empty(&self) -> bool {
        self.watts.is_empty()
    }
}

/// Power at every instant, force multiplied by velocity.
///
/// Nothing is defaulted: both registry parameters are required and carry no published
/// value, so they arrive as arguments and cannot be omitted.
pub fn instantaneous_power_watts(
    vertical_ground_reaction_force_newtons: &[f64],
    velocity: &VelocitySeries,
    system_weight_newtons: f64,
    force_term: ForceTerm,
    sign_convention: PowerSignConvention,
) -> Result<PowerSeries, PowerError> {
    if vertical_ground_reaction_force_newtons.len() != velocity.len() {
        return Err(PowerError::SeriesLengthMismatch {
            force_samples: vertical_ground_reaction_force_newtons.len(),
            velocity_samples: velocity.len(),
        });
    }
    let orientation = match sign_convention {
        PowerSignConvention::UpwardPositive => 1.0,
        PowerSignConvention::DownwardPositive => -1.0,
    };
    let watts = vertical_ground_reaction_force_newtons
        .iter()
        .zip(velocity.meters_per_second())
        .map(|(force, speed)| {
            let term = match force_term {
                ForceTerm::GroundReaction => *force,
                ForceTerm::NetOfSystemWeight => force - system_weight_newtons,
            };
            orientation * term * speed
        })
        .collect();
    Ok(PowerSeries {
        watts,
        force_term,
        sign_convention,
    })
}

/// An interval a phase model declared, carrying which model declared it.
///
/// A rate or a mean anchored to a boundary pair no model placed is a number whose phase
/// nobody can name, so the id travels with the indices rather than beside them.
#[derive(Debug, Clone, PartialEq)]
pub struct DeclaredPhase {
    pub first_index: usize,
    pub last_index: usize,
    pub method_id: String,
}

impl DeclaredPhase {
    /// The `segment`th consecutive pair of boundaries a model placed.
    pub fn from_model(
        boundaries: &PhaseModelBoundaries,
        segment: usize,
        method_id: &str,
    ) -> Result<Self, PowerError> {
        let pair_count = boundaries.indices.len().saturating_sub(1);
        if segment >= pair_count {
            return Err(PowerError::SegmentOutsideModel {
                segment,
                boundary_count: boundaries.indices.len(),
            });
        }
        Ok(Self {
            first_index: boundaries.indices[segment],
            last_index: boundaries.indices[segment + 1],
            method_id: method_id.to_string(),
        })
    }

    fn sample_count(&self) -> usize {
        self.last_index.saturating_sub(self.first_index) + 1
    }

    fn checked_against(&self, sample_count: usize) -> Result<(), PowerError> {
        if self.last_index >= sample_count || self.first_index > self.last_index {
            return Err(PowerError::PhaseOutsideTrace {
                first_index: self.first_index,
                last_index: self.last_index,
                sample_count,
            });
        }
        if self.sample_count() < 2 {
            return Err(PowerError::PhaseTooShort {
                sample_count: self.sample_count(),
            });
        }
        Ok(())
    }
}

/// The largest power reached inside a declared phase, and where.
#[derive(Debug, Clone, PartialEq)]
pub struct PeakPower {
    pub watts: f64,
    pub index: usize,
    pub phase: DeclaredPhase,
}

/// Peak instantaneous power over a declared phase.
pub fn peak_power_watts(
    series: &PowerSeries,
    phase: &DeclaredPhase,
) -> Result<PeakPower, PowerError> {
    phase.checked_against(series.len())?;
    let (index, watts) = series.watts[phase.first_index..=phase.last_index]
        .iter()
        .enumerate()
        .max_by(|left, right| {
            left.1
                .partial_cmp(right.1)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(offset, value)| (phase.first_index + offset, *value))
        .ok_or(PowerError::PhaseTooShort {
            sample_count: phase.sample_count(),
        })?;
    Ok(PeakPower {
        watts,
        index,
        phase: phase.clone(),
    })
}

/// Work in joules over a declared phase.
///
/// One integral serves both work rules. In continuous form force through a displacement and
/// power through a time are the same quantity, and the rules differ in when each is
/// selected rather than in what either computes.
pub fn work_joules(
    series: &PowerSeries,
    phase: &DeclaredPhase,
    sample_interval_seconds: f64,
) -> Result<f64, PowerError> {
    phase.checked_against(series.len())?;
    let integrated = cumulative_trapezoid(
        &series.watts[phase.first_index..=phase.last_index],
        sample_interval_seconds,
    );
    Ok(integrated.last().copied().unwrap_or(0.0))
}

/// A mean power with the interval it was taken over.
///
/// The interval is not optional decoration: a mean without it is not interpretable, and the
/// two travel together so a caller cannot report one without the other.
#[derive(Debug, Clone, PartialEq)]
pub struct MeanPower {
    pub watts: f64,
    pub duration_seconds: f64,
    pub phase: DeclaredPhase,
}

/// Mean power over a declared phase, as the work divided by the duration.
pub fn mean_power_watts(
    series: &PowerSeries,
    phase: &DeclaredPhase,
    sample_interval_seconds: f64,
) -> Result<MeanPower, PowerError> {
    let work = work_joules(series, phase, sample_interval_seconds)?;
    let duration_seconds = (phase.last_index - phase.first_index) as f64 * sample_interval_seconds;
    Ok(MeanPower {
        watts: if duration_seconds > 0.0 {
            work / duration_seconds
        } else {
            0.0
        },
        duration_seconds,
        phase: phase.clone(),
    })
}

/// Peak power estimated dimensionally from jump height and mass.
///
/// The constants are the published ones and gravity is not substituted into them: the 4.9
/// is half of the 9.8 the formula was written with, and replacing it with a declared value
/// would report a number under this rule's name that this rule does not give.
pub fn peak_power_from_height_lewis_watts(
    jump_height_meters: f64,
    system_mass_kilograms: f64,
) -> Option<f64> {
    if jump_height_meters < 0.0 || system_mass_kilograms <= 0.0 {
        return None;
    }
    Some(4.9f64.sqrt() * system_mass_kilograms * jump_height_meters.sqrt() * 9.81)
}

/// One population's regression on jump height and mass, as the registry publishes it.
///
/// The height coefficient carries its own unit because one of the ten published sets states a
/// height in metres and nine state it in centimetres, so a set read under the wrong unit is
/// wrong by a factor of a hundred. Carrying the unit beside the number is what makes that
/// unreachable rather than warned about.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HeightRegressionCoefficients {
    pub jump_height_coefficient: f64,
    /// `watts_per_centimetre` or `watts_per_metre`, as the registry spells it.
    pub jump_height_unit: &'static str,
    pub body_mass_coefficient: f64,
    pub intercept_watts: f64,
}

pub const WATTS_PER_CENTIMETRE: &str = "watts_per_centimetre";
pub const WATTS_PER_METRE: &str = "watts_per_metre";

/// Peak power from a population-calibrated regression on jump height and mass.
///
/// `None` for a unit this function does not know, rather than a height silently read in
/// metres against a coefficient calibrated per centimetre, which is the factor of a hundred
/// the registry warns about and the only way this arithmetic can be wrong.
pub fn peak_power_from_height_regression_watts(
    jump_height_meters: f64,
    system_mass_kilograms: f64,
    coefficients: &HeightRegressionCoefficients,
) -> Option<f64> {
    if !jump_height_meters.is_finite() || system_mass_kilograms <= 0.0 {
        return None;
    }
    let height = match coefficients.jump_height_unit {
        WATTS_PER_CENTIMETRE => jump_height_meters * 100.0,
        WATTS_PER_METRE => jump_height_meters,
        _ => return None,
    };
    let watts = coefficients.jump_height_coefficient * height
        + coefficients.body_mass_coefficient * system_mass_kilograms
        + coefficients.intercept_watts;
    watts.is_finite().then_some(watts)
}

/// Work as one force value multiplied by one displacement value, the vendor construction.
///
/// Registered because the majority of a generation of results were produced with it, not
/// because it is a second quadrature route. A single product equals the integral only where
/// force is constant through the displacement, which it emphatically is not during a jump, so
/// the peak taken per cycle biases the number high by an amount that depends on the shape of
/// the force-displacement curve rather than by a constant.
pub fn work_from_single_force_displacement_product_joules(
    vertical_ground_reaction_force_newtons: &[f64],
    displacement_meters: &[f64],
    phase: &DeclaredPhase,
) -> Result<f64, PowerError> {
    phase.checked_against(vertical_ground_reaction_force_newtons.len())?;
    if displacement_meters.len() != vertical_ground_reaction_force_newtons.len() {
        return Err(PowerError::SeriesLengthMismatch {
            force_samples: vertical_ground_reaction_force_newtons.len(),
            velocity_samples: displacement_meters.len(),
        });
    }
    let peak_force_newtons = vertical_ground_reaction_force_newtons
        [phase.first_index..=phase.last_index]
        .iter()
        .copied()
        .fold(f64::NEG_INFINITY, f64::max);
    let rise_meters =
        displacement_meters[phase.last_index] - displacement_meters[phase.first_index];
    Ok(peak_force_newtons * rise_meters)
}

/// A rate of power development, with the two instants the line was drawn between.
///
/// The instants travel with the number because two rules under this construct anchor
/// differently, and the slope alone cannot say which one produced it.
#[derive(Debug, Clone, PartialEq)]
pub struct PowerRate {
    pub watts_per_second: f64,
    pub first_index: usize,
    pub last_index: usize,
}

/// Rate of power development from the start of a declared phase to the instant of peak
/// power inside it.
pub fn rate_of_power_development_phase_anchored(
    series: &PowerSeries,
    phase: &DeclaredPhase,
    sample_interval_seconds: f64,
) -> Result<PowerRate, PowerError> {
    let peak = peak_power_watts(series, phase)?;
    if peak.index == phase.first_index {
        return Err(PowerError::PhaseTooShort { sample_count: 1 });
    }
    let elapsed_seconds = (peak.index - phase.first_index) as f64 * sample_interval_seconds;
    Ok(PowerRate {
        watts_per_second: (peak.watts - series.watts()[phase.first_index]) / elapsed_seconds,
        first_index: phase.first_index,
        last_index: peak.index,
    })
}

/// Rate of power development as the line from the lowest power to the highest power that
/// follows it, one value for the jump.
///
/// This takes no phase, and that is what separates it from the phase-anchored rule rather
/// than an omission: it reads the trough and the peak of the interval it is handed, so a
/// version that quietly accepted a phase would be computing the other rule under this name.
///
/// The interval is stated by the caller and is the half-open span an analysis-window rule
/// placed, never a phase. Handed the whole file instead, a recording that continues past
/// takeoff puts the trough at the landing, where force is large and centre-of-mass velocity
/// is negative, and the line is then drawn between two instants of the landing rather than of
/// the jump. That is the untrimmed-recording defect this project was founded on.
pub fn rate_of_power_development_peak_to_peak(
    series: &PowerSeries,
    search_start: usize,
    search_end: usize,
    sample_interval_seconds: f64,
) -> Result<PowerRate, PowerError> {
    let watts = series.watts();
    let end = search_end.min(watts.len());
    if search_start >= end || end - search_start < 2 {
        return Err(PowerError::PhaseTooShort {
            sample_count: end.saturating_sub(search_start),
        });
    }
    let inside = |(offset, value): (usize, &f64)| (search_start + offset, *value);
    let (trough_index, trough) = watts[search_start..end]
        .iter()
        .enumerate()
        .min_by(|left, right| {
            left.1
                .partial_cmp(right.1)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(inside)
        .ok_or(PowerError::PhaseTooShort { sample_count: 0 })?;
    // The peak has to follow the trough, which is what "subsequent" states and what makes
    // the slope a rise rather than whichever of the two happened to come first.
    let (peak_index, peak) = watts[trough_index + 1..end]
        .iter()
        .enumerate()
        .max_by(|left, right| {
            left.1
                .partial_cmp(right.1)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(offset, value)| (trough_index + 1 + offset, *value))
        .ok_or(PowerError::PhaseTooShort {
            sample_count: end - trough_index,
        })?;
    let elapsed_seconds = (peak_index - trough_index) as f64 * sample_interval_seconds;
    Ok(PowerRate {
        watts_per_second: (peak - trough) / elapsed_seconds,
        first_index: trough_index,
        last_index: peak_index,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::statistics::compensated_sum;

    /// The average power over a phase taken sample by sample rather than as work over duration.
    ///
    /// Present so the equivalence the rule asserts can be measured on real data rather than
    /// taken on trust. It is not a second method and no registry id resolves to it.
    fn mean_of_samples_watts(series: &PowerSeries, phase: &DeclaredPhase) -> f64 {
        let samples = &series.watts[phase.first_index..=phase.last_index];
        compensated_sum(samples) / samples.len() as f64
    }

    use crate::series::{
        centre_of_mass_velocity_meters_per_second, IntegrationAnchor, IntegrationDirection,
        IntegrationSpec, IntegrationStart, QuadratureRule,
    };
    use crate::trial::WeighingEpoch;
    use crate::Trial;

    const SAMPLE_RATE_HZ: f64 = 1000.0;
    const SYSTEM_WEIGHT_NEWTONS: f64 = 700.0;

    fn spec() -> IntegrationSpec {
        IntegrationSpec {
            quadrature: QuadratureRule::Trapezoid,
            direction: IntegrationDirection::Forward,
            start: IntegrationStart::TrialStart,
            anchor: IntegrationAnchor::SinglePoint { index: 0 },
        }
    }

    fn epoch(sample_count: usize) -> WeighingEpoch {
        WeighingEpoch {
            start_index: 0,
            end_index: sample_count.min(100),
            system_weight_newtons: SYSTEM_WEIGHT_NEWTONS,
            standard_deviation_newtons: 1.0,
            tied_window_count: 1,
            tied_weight_low_newtons: SYSTEM_WEIGHT_NEWTONS,
            tied_weight_high_newtons: SYSTEM_WEIGHT_NEWTONS,
        }
    }

    /// A trace that stands still, unweights, then pushes, so velocity crosses zero and power
    /// takes both signs.
    fn jump_like_force() -> Vec<f64> {
        (0..1200)
            .map(|sample| match sample {
                0..=199 => SYSTEM_WEIGHT_NEWTONS,
                200..=499 => SYSTEM_WEIGHT_NEWTONS - 250.0,
                500..=899 => SYSTEM_WEIGHT_NEWTONS + 400.0,
                _ => 0.0,
            })
            .collect()
    }

    fn trial_from(force: Vec<f64>) -> Trial {
        Trial::new(force, SAMPLE_RATE_HZ).unwrap()
    }

    fn power_from(force: &[f64], term: ForceTerm) -> PowerSeries {
        let trial = trial_from(force.to_vec());
        let velocity =
            centre_of_mass_velocity_meters_per_second(&trial, &epoch(force.len()), &spec(), 9.81);
        instantaneous_power_watts(
            trial.force(),
            &velocity,
            SYSTEM_WEIGHT_NEWTONS,
            term,
            PowerSignConvention::UpwardPositive,
        )
        .unwrap()
    }

    #[test]
    fn a_power_series_carries_the_two_choices_that_produced_it() {
        let series = power_from(&jump_like_force(), ForceTerm::GroundReaction);
        assert_eq!(series.force_term(), ForceTerm::GroundReaction);
        assert_eq!(
            series.sign_convention(),
            PowerSignConvention::UpwardPositive
        );
        assert_eq!(series.len(), 1200);
    }

    #[test]
    fn the_two_force_terms_give_different_power_by_one_system_weight_times_velocity() {
        let force = jump_like_force();
        let gross = power_from(&force, ForceTerm::GroundReaction);
        let net = power_from(&force, ForceTerm::NetOfSystemWeight);
        let trial = trial_from(force.clone());
        let velocity =
            centre_of_mass_velocity_meters_per_second(&trial, &epoch(force.len()), &spec(), 9.81);
        let mut largest_gap = 0.0f64;
        for (index, speed) in velocity.meters_per_second().iter().enumerate() {
            let expected = SYSTEM_WEIGHT_NEWTONS * speed;
            largest_gap =
                largest_gap.max(((gross.watts()[index] - net.watts()[index]) - expected).abs());
        }
        assert!(largest_gap < 1e-9, "the two terms are not one weight apart");
        // And the choice has to reach the number, not only the record.
        let moved = gross
            .watts()
            .iter()
            .zip(net.watts())
            .any(|(left, right)| (left - right).abs() > 1.0);
        assert!(moved, "the force term changed nothing");
    }

    #[test]
    fn the_sign_convention_reverses_the_series() {
        let force = jump_like_force();
        let trial = trial_from(force.clone());
        let velocity =
            centre_of_mass_velocity_meters_per_second(&trial, &epoch(force.len()), &spec(), 9.81);
        let up = instantaneous_power_watts(
            trial.force(),
            &velocity,
            SYSTEM_WEIGHT_NEWTONS,
            ForceTerm::GroundReaction,
            PowerSignConvention::UpwardPositive,
        )
        .unwrap();
        let down = instantaneous_power_watts(
            trial.force(),
            &velocity,
            SYSTEM_WEIGHT_NEWTONS,
            ForceTerm::GroundReaction,
            PowerSignConvention::DownwardPositive,
        )
        .unwrap();
        for (left, right) in up.watts().iter().zip(down.watts()) {
            assert!((left + right).abs() < 1e-12);
        }
        assert!(up.watts().iter().any(|value| value.abs() > 1.0));
    }

    #[test]
    fn a_mismatched_velocity_series_is_named_rather_than_truncated() {
        let short = trial_from(vec![SYSTEM_WEIGHT_NEWTONS; 400]);
        let velocity =
            centre_of_mass_velocity_meters_per_second(&short, &epoch(400), &spec(), 9.81);
        let error = instantaneous_power_watts(
            &vec![SYSTEM_WEIGHT_NEWTONS; 500],
            &velocity,
            SYSTEM_WEIGHT_NEWTONS,
            ForceTerm::GroundReaction,
            PowerSignConvention::UpwardPositive,
        )
        .unwrap_err();
        assert!(matches!(
            error,
            PowerError::SeriesLengthMismatch {
                force_samples: 500,
                velocity_samples: 400
            }
        ));
    }

    fn declared(first_index: usize, last_index: usize) -> DeclaredPhase {
        DeclaredPhase {
            first_index,
            last_index,
            method_id: "phase.model.unweighting_single.mcmahon2018".to_string(),
        }
    }

    #[test]
    fn work_over_a_constant_power_interval_is_the_closed_form_answer() {
        // Constant force through constant velocity for a known time. Ten newtons at two
        // metres per second for one second is twenty joules, and nothing about the
        // implementation is consulted to know that.
        let series = PowerSeries {
            watts: vec![20.0; 1001],
            force_term: ForceTerm::GroundReaction,
            sign_convention: PowerSignConvention::UpwardPositive,
        };
        let phase = declared(0, 1000);
        let work = work_joules(&series, &phase, 1.0 / 1000.0).unwrap();
        assert!((work - 20.0).abs() < 1e-9, "work came back {work}");
        let mean = mean_power_watts(&series, &phase, 1.0 / 1000.0).unwrap();
        assert!((mean.watts - 20.0).abs() < 1e-9);
        assert!((mean.duration_seconds - 1.0).abs() < 1e-12);
    }

    #[test]
    fn a_ramp_integrates_to_the_area_under_it() {
        // A straight ramp from zero to a thousand watts over one second is 500 joules, and
        // the trapezoid is exact on a straight line.
        let watts: Vec<f64> = (0..=1000).map(|sample| sample as f64).collect();
        let series = PowerSeries {
            watts,
            force_term: ForceTerm::GroundReaction,
            sign_convention: PowerSignConvention::UpwardPositive,
        };
        let work = work_joules(&series, &declared(0, 1000), 1.0 / 1000.0).unwrap();
        assert!((work - 500.0).abs() < 1e-9, "work came back {work}");
    }

    #[test]
    fn the_mean_matches_the_sample_average_to_quadrature_error() {
        // The rule states the two forms are one quantity differing only by quadrature, so
        // that is measured here rather than assumed. The trapezoid half-weights the two end
        // samples, which is the whole of the difference.
        let series = power_from(&jump_like_force(), ForceTerm::GroundReaction);
        let phase = declared(200, 900);
        let as_work = mean_power_watts(&series, &phase, 1.0 / SAMPLE_RATE_HZ).unwrap();
        let as_samples = mean_of_samples_watts(&series, &phase);
        let gap = (as_work.watts - as_samples).abs();
        assert!(
            gap / as_samples.abs() < 0.01,
            "{} against {}",
            as_work.watts,
            as_samples
        );
    }

    #[test]
    fn peak_power_is_the_peak_inside_its_phase_and_not_the_peak_of_the_trace() {
        let series = power_from(&jump_like_force(), ForceTerm::GroundReaction);
        // Unweighting holds force below system weight while velocity is negative;
        // propulsion holds it above while velocity is positive. The largest power on the
        // trace is in the second, so a peak that quietly scanned the whole series would
        // return one number for both phases and every assertion about bounds would still
        // hold.
        let unweighting = peak_power_watts(&series, &declared(200, 499)).unwrap();
        let propulsion = peak_power_watts(&series, &declared(500, 899)).unwrap();
        assert!(
            unweighting.watts < propulsion.watts,
            "the phase reached nothing: {} against {}",
            unweighting.watts,
            propulsion.watts
        );
        assert!(
            (200..=499).contains(&unweighting.index),
            "the peak was found at {} and the phase ran 200 to 499",
            unweighting.index
        );
        assert!((500..=899).contains(&propulsion.index));
        assert_eq!(
            propulsion.phase.method_id,
            "phase.model.unweighting_single.mcmahon2018"
        );
    }

    #[test]
    fn a_phase_the_trace_does_not_contain_is_refused() {
        let series = power_from(&jump_like_force(), ForceTerm::GroundReaction);
        assert!(matches!(
            peak_power_watts(&series, &declared(1000, 5000)).unwrap_err(),
            PowerError::PhaseOutsideTrace { .. }
        ));
        assert!(matches!(
            work_joules(&series, &declared(500, 500), 0.001).unwrap_err(),
            PowerError::PhaseTooShort { .. }
        ));
    }

    #[test]
    fn a_phase_comes_from_a_model_that_placed_it() {
        let boundaries = PhaseModelBoundaries {
            indices: vec![100, 400, 900],
        };
        let first = DeclaredPhase::from_model(
            &boundaries,
            0,
            "phase.model.unloading_yielding_split.harry2020",
        )
        .unwrap();
        assert_eq!(first.first_index, 100);
        assert_eq!(first.last_index, 400);
        assert_eq!(
            first.method_id,
            "phase.model.unloading_yielding_split.harry2020"
        );
        let second = DeclaredPhase::from_model(&boundaries, 1, "x").unwrap();
        assert_eq!((second.first_index, second.last_index), (400, 900));
        assert!(matches!(
            DeclaredPhase::from_model(&boundaries, 2, "x").unwrap_err(),
            PowerError::SegmentOutsideModel { segment: 2, .. }
        ));
    }

    #[test]
    fn the_phase_anchored_rate_runs_from_the_phase_start_to_the_peak_inside_it() {
        let series = power_from(&jump_like_force(), ForceTerm::GroundReaction);
        let rate = rate_of_power_development_phase_anchored(
            &series,
            &declared(500, 899),
            1.0 / SAMPLE_RATE_HZ,
        )
        .unwrap();
        assert_eq!(rate.first_index, 500);
        assert!((500..=899).contains(&rate.last_index));
        assert!(rate.watts_per_second > 0.0);

        // Moving the phase start moves the line, so the declared phase reaches the number
        // rather than riding along beside it.
        let earlier = rate_of_power_development_phase_anchored(
            &series,
            &declared(400, 899),
            1.0 / SAMPLE_RATE_HZ,
        )
        .unwrap();
        assert_eq!(earlier.first_index, 400);
        assert_ne!(earlier.watts_per_second, rate.watts_per_second);
    }

    #[test]
    fn a_phase_whose_peak_is_its_own_first_sample_is_refused_rather_than_reported_as_zero() {
        // Through the unweighting stretch power only falls, so the largest value inside it
        // is the sample it starts on and the rule has no line to draw. A slope over no
        // elapsed time is not a rate, and reporting one would be a number with no method.
        let series = power_from(&jump_like_force(), ForceTerm::GroundReaction);
        assert!(matches!(
            rate_of_power_development_phase_anchored(
                &series,
                &declared(200, 499),
                1.0 / SAMPLE_RATE_HZ
            )
            .unwrap_err(),
            PowerError::PhaseTooShort { .. }
        ));
    }

    #[test]
    fn the_peak_to_peak_rate_reads_every_sample_of_the_interval_it_is_handed() {
        let force = jump_like_force();
        let series = power_from(&force, ForceTerm::GroundReaction);
        let whole = series.len();
        let rate = rate_of_power_development_peak_to_peak(&series, 0, whole, 1.0 / SAMPLE_RATE_HZ)
            .unwrap();
        assert!(rate.watts_per_second > 0.0);

        // Deepening the trough late in the trace moves the answer. A rule scoped to any one
        // phase would return the same number for both recordings, which is the difference
        // between this rule and the phase-anchored one beside it.
        let mut deeper = force.clone();
        for sample in deeper.iter_mut().take(720).skip(700) {
            *sample += 3000.0;
        }
        let deepened = power_from(&deeper, ForceTerm::GroundReaction);
        let moved =
            rate_of_power_development_peak_to_peak(&deepened, 0, whole, 1.0 / SAMPLE_RATE_HZ)
                .unwrap();
        assert_ne!(moved.watts_per_second, rate.watts_per_second);
    }

    /// The interval is the whole of what keeps a landing out of the answer, so a trough
    /// outside it must not reach the line.
    #[test]
    fn the_interval_bounds_the_peak_to_peak_search() {
        let series = PowerSeries {
            watts: vec![100.0, -50.0, 300.0, -9000.0, 4000.0],
            force_term: ForceTerm::GroundReaction,
            sign_convention: PowerSignConvention::UpwardPositive,
        };
        let inside = rate_of_power_development_peak_to_peak(&series, 0, 3, 0.001).unwrap();
        assert_eq!(inside.first_index, 1);
        assert_eq!(inside.last_index, 2);

        // The same series read to its end takes the far deeper trough and the far higher
        // peak after it, which is a different number entirely.
        let across = rate_of_power_development_peak_to_peak(&series, 0, 5, 0.001).unwrap();
        assert_eq!(across.first_index, 3);
        assert_eq!(across.last_index, 4);
        assert!(across.watts_per_second > inside.watts_per_second);
    }

    #[test]
    fn the_peak_to_peak_rate_takes_the_peak_that_follows_the_trough() {
        // A series whose largest value precedes its smallest. Taking the global maximum
        // would give a negative elapsed time or a backwards line.
        let series = PowerSeries {
            watts: vec![500.0, 100.0, -300.0, -50.0, 200.0],
            force_term: ForceTerm::GroundReaction,
            sign_convention: PowerSignConvention::UpwardPositive,
        };
        let rate = rate_of_power_development_peak_to_peak(&series, 0, 5, 0.001).unwrap();
        assert_eq!(rate.first_index, 2);
        assert_eq!(rate.last_index, 4);
        assert!(rate.watts_per_second > 0.0);
    }

    #[test]
    fn a_series_whose_trough_is_its_last_sample_is_refused() {
        let series = PowerSeries {
            watts: vec![5.0, 3.0, -9.0],
            force_term: ForceTerm::GroundReaction,
            sign_convention: PowerSignConvention::UpwardPositive,
        };
        assert!(matches!(
            rate_of_power_development_peak_to_peak(&series, 0, 3, 0.001).unwrap_err(),
            PowerError::PhaseTooShort { .. }
        ));
    }

    /// A set stating its height coefficient per metre and a set stating it per centimetre
    /// give answers of the same order on one jump.
    ///
    /// The registry's own warning is that a set read under the wrong unit is wrong by a factor
    /// of a hundred, and that is the only way this arithmetic can fail: every term is a
    /// multiplication a reader could check by hand. So the unit is carried beside the
    /// coefficient and read here, rather than applied by whoever calls this.
    #[test]
    fn a_regression_set_is_read_under_the_height_unit_it_states() {
        let per_centimetre = HeightRegressionCoefficients {
            jump_height_coefficient: 61.9,
            jump_height_unit: WATTS_PER_CENTIMETRE,
            body_mass_coefficient: 36.0,
            intercept_watts: -1822.0,
        };
        // Shetty's line, which is the one set of the ten stating a height in metres. Its
        // height coefficient is very nearly a hundred times a per-centimetre one, which is
        // what makes the two readings indistinguishable by size.
        let per_metre = HeightRegressionCoefficients {
            jump_height_coefficient: 1925.72,
            jump_height_unit: WATTS_PER_METRE,
            body_mass_coefficient: 14.74,
            intercept_watts: -666.3,
        };
        let height_meters = 0.44;
        let mass = 59.88;
        let centimetre_set =
            peak_power_from_height_regression_watts(height_meters, mass, &per_centimetre).unwrap();
        let metre_set =
            peak_power_from_height_regression_watts(height_meters, mass, &per_metre).unwrap();
        assert!(
            (metre_set / centimetre_set - 1.0).abs() < 1.0,
            "{metre_set} W against {centimetre_set} W, which is the factor of a hundred the \
             unit exists to prevent"
        );

        // Read under the wrong unit the same coefficients answer a hundred times apart, so
        // the guard above is measuring the unit rather than the coefficients happening to be
        // close.
        let mislabelled = HeightRegressionCoefficients {
            jump_height_unit: WATTS_PER_CENTIMETRE,
            ..per_metre
        };
        let wrong =
            peak_power_from_height_regression_watts(height_meters, mass, &mislabelled).unwrap();
        assert!(
            wrong / metre_set > 50.0,
            "reading the per-metre set per centimetre moved it from {metre_set} to {wrong}"
        );

        // A unit nothing knows is nothing rather than a height read under a guess.
        let unknown = HeightRegressionCoefficients {
            jump_height_unit: "watts_per_furlong",
            ..per_centimetre
        };
        assert!(peak_power_from_height_regression_watts(height_meters, mass, &unknown).is_none());
        assert!(
            peak_power_from_height_regression_watts(height_meters, 0.0, &per_centimetre).is_none()
        );
    }

    /// The vendor product is the peak force through the phase's rise, and it is not the
    /// integral.
    ///
    /// Where force is constant the two agree exactly, which is the identity the construction
    /// rests on. Where it is not, the product takes the peak against every sample and comes
    /// out high, and the size of that is a property of the curve rather than a constant.
    #[test]
    fn the_single_product_equals_the_integral_only_where_force_is_constant() {
        let constant = vec![1000.0; 501];
        let displacement: Vec<f64> = (0..=500).map(|sample| sample as f64 * 0.001).collect();
        let phase = declared(0, 500);
        let product =
            work_from_single_force_displacement_product_joules(&constant, &displacement, &phase)
                .unwrap();
        // A thousand newtons through half a metre is five hundred joules, known without
        // consulting the implementation.
        assert!((product - 500.0).abs() < 1e-9, "came back {product}");

        // A force that rises linearly through the same displacement does half that work, and
        // the product takes the peak against all of it, so it reads double the truth.
        let rising: Vec<f64> = (0..=500).map(|sample| sample as f64 * 2.0).collect();
        let over_a_ramp =
            work_from_single_force_displacement_product_joules(&rising, &displacement, &phase)
                .unwrap();
        assert!(
            (over_a_ramp - 500.0).abs() < 1e-9,
            "the peak of the ramp is 1000 N through 0.5 m: {over_a_ramp}"
        );
        let truth = 500.0f64 * 0.5; // the mean of the ramp through the same rise
        assert!(
            over_a_ramp / truth > 1.9,
            "the product came to {over_a_ramp} J against the {truth} J the integral gives, \
             which is not the bias the registry records"
        );

        // A phase the trace does not hold, and a displacement of a different length, are both
        // named rather than truncated.
        assert!(work_from_single_force_displacement_product_joules(
            &constant,
            &displacement,
            &declared(0, 900)
        )
        .is_err());
        assert!(matches!(
            work_from_single_force_displacement_product_joules(
                &constant,
                &displacement[..100],
                &phase
            ),
            Err(PowerError::SeriesLengthMismatch { .. })
        ));
    }

    #[test]
    fn the_lewis_estimate_scales_as_the_root_of_height() {
        // Quadrupling the height doubles the estimate, which is the dimensional content of
        // the formula and is checkable without the paper.
        let low = peak_power_from_height_lewis_watts(0.1, 80.0).unwrap();
        let high = peak_power_from_height_lewis_watts(0.4, 80.0).unwrap();
        assert!((high / low - 2.0).abs() < 1e-12);
        // And it is linear in mass.
        let heavier = peak_power_from_height_lewis_watts(0.1, 160.0).unwrap();
        assert!((heavier / low - 2.0).abs() < 1e-12);
        assert!(peak_power_from_height_lewis_watts(-0.1, 80.0).is_none());
        assert!(peak_power_from_height_lewis_watts(0.1, 0.0).is_none());
    }
}
