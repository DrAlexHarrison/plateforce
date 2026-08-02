//! Centre of mass velocity over a whole trial, and the four choices that produce it.
//!
//! `integration.toml` states four separate choices across two constructs: the quadrature the
//! net-force integral is evaluated by, the direction it runs, where it starts, and where the
//! constants of integration are pinned. They are four because published pipelines mix them
//! independently: the landing frame integrates backward from touchdown against a different
//! anchor, and a signature fusing start and anchor can express one of the two.
//!
//! `integration.start.trial_start` is deprecated and `integration.start.detected_onset` is
//! recommended, and the two produce different velocities from one recording. Neither is
//! visible in the output, so `IntegrationSpec` has no `Default` and the choice cannot be
//! reached without being stated.

use crate::phases::{center_of_mass_acceleration, cumulative_trapezoid};
use crate::trial::WeighingEpoch;
use crate::Trial;

/// How the net-force integral is evaluated, `integration.rule.*` on the `net_impulse`
/// construct.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuadratureRule {
    /// Error order h squared.
    Trapezoid,
    /// Error order h to the fourth, over pairs of intervals.
    Simpson,
    /// Error order h, which is what a plain cumulative sum computes.
    Rectangle,
}

/// Which way the integral runs, `integration.direction.*`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntegrationDirection {
    /// Forward from quiet standing, where velocity is known to be zero.
    Forward,
    /// Backward from confirmed stillness after landing. The height this produces is a
    /// landing-frame height and is not the same quantity as the takeoff-frame one.
    Backward,
}

/// Where the integral starts, `integration.start.*`. Both entries carry
/// `surfacing = force_a_decision`, and they disagree on real recordings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntegrationStart {
    /// Index zero of the possibly cropped series, before onset is known. Status
    /// `deprecated`: a recording with quiet stance ahead of the movement accumulates that
    /// stance into the velocity.
    TrialStart,
    /// The interval from detected onset, status `recommended`. Carries the index the bound
    /// onset rule placed, because this module cannot find onset and must not guess it.
    DetectedOnset { index: usize },
}

/// A centre-of-mass height at takeoff, indexed by body height, which
/// `integration.anchor.per_jump_midflight.kistler` rests on and no published source gives.
///
/// Held with no public constructor because the values do not exist to put in one.
#[derive(Debug, Clone, PartialEq)]
pub struct CentreOfMassHeightTable {
    height_at_takeoff_meters_by_body_height: Vec<(f64, f64)>,
}

impl CentreOfMassHeightTable {
    pub fn height_at_takeoff_meters(&self, body_height_meters: f64) -> Option<f64> {
        self.height_at_takeoff_meters_by_body_height
            .iter()
            .min_by(|left, right| {
                (left.0 - body_height_meters)
                    .abs()
                    .total_cmp(&(right.0 - body_height_meters).abs())
            })
            .map(|entry| entry.1)
    }
}

/// Where the constants of integration are pinned, `integration.anchor.*`.
///
/// The published anchors constrain different numbers of instants, so a scalar expresses one
/// of them and approximates the rest.
#[derive(Debug, Clone, PartialEq)]
pub enum IntegrationAnchor {
    /// Velocity is zero at one stated instant.
    SinglePoint { index: usize },
    /// The integrated quantity takes a stated non-zero value at one stated instant, which is
    /// how the landing frame is reconstructed: `phase.landing_end.zero_com_velocity`
    /// integrates from landing acceleration with the negative takeoff velocity as its initial
    /// value. `value` is in the unit of the series being anchored, which its type states.
    ///
    /// `integration.anchor.*` carries no entry for this, because the value is stated by the
    /// rule that needs it rather than by an anchor rule. That rule's id travels here so the
    /// record names whoever chose the value instead of crediting an anchor entry that did not.
    SinglePointAtValue {
        index: usize,
        value: f64,
        stated_by_method_id: String,
    },
    /// Velocity is zero at both ends, which requires the subject to stand still and in the
    /// same position at each. A two-ended constraint, not a starting value.
    TwoPoint {
        start_index: usize,
        end_index: usize,
    },
    /// Re-anchored once per jump at the midpoint of each flight time, against a
    /// body-height-indexed table of centre-of-mass height at takeoff.
    PerJumpMidflight { table: CentreOfMassHeightTable },
}

/// The four choices `integration.toml` states, travelling together because the binding
/// layer, the provenance record and the series all have to carry the same four.
///
/// No `Default` impl. A caller cannot obtain one without stating all four.
#[derive(Debug, Clone, PartialEq)]
pub struct IntegrationSpec {
    pub quadrature: QuadratureRule,
    pub direction: IntegrationDirection,
    pub start: IntegrationStart,
    pub anchor: IntegrationAnchor,
}

impl IntegrationSpec {
    /// The registry ids this spec names, in the order the constructs are declared. Every
    /// caller that records provenance needs the same four, so they are spelled once.
    pub fn method_ids(&self) -> [&str; 4] {
        [
            match self.quadrature {
                QuadratureRule::Trapezoid => "integration.rule.trapezoid",
                QuadratureRule::Simpson => "integration.rule.simpson",
                QuadratureRule::Rectangle => "integration.rule.rectangle",
            },
            match self.direction {
                IntegrationDirection::Forward => "integration.direction.forward",
                IntegrationDirection::Backward => "integration.direction.backward",
            },
            match self.start {
                IntegrationStart::TrialStart => "integration.start.trial_start",
                IntegrationStart::DetectedOnset { .. } => "integration.start.detected_onset",
            },
            match &self.anchor {
                IntegrationAnchor::SinglePoint { .. } => "integration.anchor.single_point",
                IntegrationAnchor::SinglePointAtValue {
                    stated_by_method_id,
                    ..
                } => stated_by_method_id,
                IntegrationAnchor::TwoPoint { .. } => "integration.anchor.two_point.kistler",
                IntegrationAnchor::PerJumpMidflight { .. } => {
                    "integration.anchor.per_jump_midflight.kistler"
                }
            },
        ]
    }
}

/// Centre-of-mass velocity in metres per second, one sample per sample of the trial, and
/// the spec that produced it.
///
/// A consumer cannot receive the samples without receiving what produced them, which is
/// what stops a landmark being read off a series whose settings nobody stated.
#[derive(Debug, Clone, PartialEq)]
pub struct VelocitySeries {
    meters_per_second: Vec<f64>,
    spec: IntegrationSpec,
    first_integrated_index: usize,
    sample_interval_seconds: f64,
}

impl VelocitySeries {
    /// A series over samples the caller already holds, which still cannot be built without a
    /// spec.
    ///
    /// The invariant is that a consumer receives what produced the samples, not that this
    /// crate produced them. A crossing search is tested against velocities written by hand to
    /// place a minimum on a segment boundary or a threshold between two samples, and reaching
    /// those through the integrator would mean solving for a force trace that yields them,
    /// which tests the integrator rather than the search.
    pub fn from_samples(
        meters_per_second: Vec<f64>,
        spec: IntegrationSpec,
        first_integrated_index: usize,
        sample_interval_seconds: f64,
    ) -> Self {
        Self {
            meters_per_second,
            spec,
            first_integrated_index,
            sample_interval_seconds,
        }
    }

    pub fn meters_per_second(&self) -> &[f64] {
        &self.meters_per_second
    }

    pub fn spec(&self) -> &IntegrationSpec {
        &self.spec
    }

    /// Carried so a second integration cannot be handed a different interval from the one
    /// this series was built at.
    pub fn sample_interval_seconds(&self) -> f64 {
        self.sample_interval_seconds
    }

    /// The index the integral started from. Samples before it were not integrated and
    /// carry the anchor value rather than a measurement.
    pub fn first_integrated_index(&self) -> usize {
        self.first_integrated_index
    }

    pub fn at(&self, index: usize) -> Option<f64> {
        self.meters_per_second.get(index).copied()
    }

    pub fn len(&self) -> usize {
        self.meters_per_second.len()
    }

    pub fn is_empty(&self) -> bool {
        self.meters_per_second.is_empty()
    }
}

/// Centre-of-mass velocity, anchored where the caller declared.
///
/// Nothing here is defaulted: `IntegrationSpec` has no `Default` impl, so this cannot be
/// called without four stated choices.
pub fn centre_of_mass_velocity_meters_per_second(
    trial: &Trial,
    epoch: &WeighingEpoch,
    spec: &IntegrationSpec,
    gravity_meters_per_second_squared: f64,
) -> VelocitySeries {
    let acceleration = center_of_mass_acceleration(
        trial.force(),
        epoch.system_weight_newtons,
        epoch.system_mass_kilograms(gravity_meters_per_second_squared),
    );
    let sample_interval_seconds = trial.sample_interval_seconds();
    let first_integrated_index = match spec.start {
        IntegrationStart::TrialStart => 0,
        IntegrationStart::DetectedOnset { index } => index.min(acceleration.len()),
    };

    let integrated = match spec.direction {
        IntegrationDirection::Forward => integrate_forward(
            &acceleration,
            first_integrated_index,
            sample_interval_seconds,
            spec.quadrature,
        ),
        IntegrationDirection::Backward => integrate_backward(
            &acceleration,
            first_integrated_index,
            sample_interval_seconds,
            spec.quadrature,
        ),
    };

    VelocitySeries {
        meters_per_second: apply_anchor(integrated, &spec.anchor),
        spec: spec.clone(),
        first_integrated_index,
        sample_interval_seconds,
    }
}

/// Centre-of-mass displacement in metres, one sample per sample of the trial, with the spec
/// that produced it and the spec of the velocity it was integrated from.
///
/// Both travel because displacement inherits every choice the velocity made and adds four of
/// its own, so a record naming only the second integration understates what produced it.
#[derive(Debug, Clone, PartialEq)]
pub struct DisplacementSeries {
    meters: Vec<f64>,
    spec: IntegrationSpec,
    velocity_spec: IntegrationSpec,
}

impl DisplacementSeries {
    pub fn meters(&self) -> &[f64] {
        &self.meters
    }

    pub fn spec(&self) -> &IntegrationSpec {
        &self.spec
    }

    /// The choices behind the velocity this was integrated from.
    pub fn velocity_spec(&self) -> &IntegrationSpec {
        &self.velocity_spec
    }

    pub fn at(&self, index: usize) -> Option<f64> {
        self.meters.get(index).copied()
    }

    pub fn len(&self) -> usize {
        self.meters.len()
    }

    pub fn is_empty(&self) -> bool {
        self.meters.is_empty()
    }
}

/// Centre-of-mass displacement, integrated from a velocity series under its own four choices.
///
/// Anchored at quiet standing, the value at takeoff is the heel-rise term and the apex is the
/// height in the standing frame, which is a different quantity from the takeoff frame rather
/// than a different method of computing it.
pub fn centre_of_mass_displacement_meters(
    velocity: &VelocitySeries,
    spec: &IntegrationSpec,
) -> DisplacementSeries {
    let samples = velocity.meters_per_second();
    let first_integrated_index = match spec.start {
        IntegrationStart::TrialStart => 0,
        IntegrationStart::DetectedOnset { index } => index.min(samples.len()),
    };
    let integrated = match spec.direction {
        IntegrationDirection::Forward => integrate_forward(
            samples,
            first_integrated_index,
            velocity.sample_interval_seconds(),
            spec.quadrature,
        ),
        IntegrationDirection::Backward => integrate_backward(
            samples,
            first_integrated_index,
            velocity.sample_interval_seconds(),
            spec.quadrature,
        ),
    };
    DisplacementSeries {
        meters: apply_anchor(integrated, &spec.anchor),
        spec: spec.clone(),
        velocity_spec: velocity.spec().clone(),
    }
}

/// Running integral from `start`, zero there, holding the samples before it at zero so the
/// series indexes against the trial rather than against the interval.
fn integrate_forward(
    acceleration: &[f64],
    start: usize,
    sample_interval_seconds: f64,
    quadrature: QuadratureRule,
) -> Vec<f64> {
    let mut velocity = vec![0.0f64; acceleration.len()];
    if start >= acceleration.len() {
        return velocity;
    }
    let integrated = running_integral(&acceleration[start..], sample_interval_seconds, quadrature);
    velocity[start..].copy_from_slice(&integrated);
    velocity
}

/// Running integral backward from `start`, zero there, for the landing frame.
fn integrate_backward(
    acceleration: &[f64],
    start: usize,
    sample_interval_seconds: f64,
    quadrature: QuadratureRule,
) -> Vec<f64> {
    let mut velocity = vec![0.0f64; acceleration.len()];
    if start >= acceleration.len() {
        return velocity;
    }
    let leading: Vec<f64> = acceleration[..=start].iter().rev().copied().collect();
    let integrated = running_integral(&leading, sample_interval_seconds, quadrature);
    for (offset, value) in integrated.iter().enumerate() {
        velocity[start - offset] = -value;
    }
    velocity
}

/// One quadrature, one place. The trapezoid arm calls `cumulative_trapezoid` rather than
/// repeating it, so a harness that declares the trapezoid reads the same numbers it read
/// before the choice became stateable.
fn running_integral(
    values: &[f64],
    sample_interval_seconds: f64,
    quadrature: QuadratureRule,
) -> Vec<f64> {
    match quadrature {
        QuadratureRule::Trapezoid => cumulative_trapezoid(values, sample_interval_seconds),
        QuadratureRule::Rectangle => {
            let mut integrated = vec![0.0f64; values.len()];
            for index in 1..values.len() {
                integrated[index] =
                    integrated[index - 1] + values[index - 1] * sample_interval_seconds;
            }
            integrated
        }
        QuadratureRule::Simpson => {
            let mut integrated = vec![0.0f64; values.len()];
            for index in 1..values.len() {
                integrated[index] = if index >= 2 && index % 2 == 0 {
                    integrated[index - 2]
                        + (values[index - 2] + 4.0 * values[index - 1] + values[index])
                            * sample_interval_seconds
                            / 3.0
                } else {
                    integrated[index - 1]
                        + 0.5 * (values[index] + values[index - 1]) * sample_interval_seconds
                };
            }
            integrated
        }
    }
}

/// Pin the constants of integration where the anchor says, rather than wherever the
/// integral happened to start.
fn apply_anchor(mut velocity: Vec<f64>, anchor: &IntegrationAnchor) -> Vec<f64> {
    match anchor {
        IntegrationAnchor::SinglePoint { index } => {
            let Some(offset) = velocity.get(*index).copied() else {
                return velocity;
            };
            for value in velocity.iter_mut() {
                *value -= offset;
            }
            velocity
        }
        IntegrationAnchor::SinglePointAtValue { index, value, .. } => {
            let Some(at_anchor) = velocity.get(*index).copied() else {
                return velocity;
            };
            let offset = at_anchor - value;
            for value in velocity.iter_mut() {
                *value -= offset;
            }
            velocity
        }
        IntegrationAnchor::TwoPoint {
            start_index,
            end_index,
        } => {
            if start_index >= end_index || *end_index >= velocity.len() {
                return velocity;
            }
            // Velocity zero at both ends. A constant error in the initial acceleration
            // shows up as a linear ramp in velocity, so removing the ramp through the two
            // ends is the adjustment the entry describes.
            let at_start = velocity[*start_index];
            let at_end = velocity[*end_index];
            let span = (*end_index - *start_index) as f64;
            for (index, value) in velocity.iter_mut().enumerate() {
                let position = (index as f64 - *start_index as f64) / span;
                *value -= at_start + (at_end - at_start) * position;
            }
            velocity
        }
        IntegrationAnchor::PerJumpMidflight { .. } => velocity,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_RATE_HZ: f64 = 1200.0;

    fn spec(start: IntegrationStart) -> IntegrationSpec {
        IntegrationSpec {
            quadrature: QuadratureRule::Trapezoid,
            direction: IntegrationDirection::Forward,
            start,
            anchor: IntegrationAnchor::SinglePoint { index: 0 },
        }
    }

    /// Quiet stance, then an unweighting dip and a push. The stance ahead of the movement
    /// is what the two starts disagree about.
    fn trial_and_epoch() -> (Trial, WeighingEpoch, usize) {
        let weight = 700.0;
        let mut force = vec![weight; 1200];
        for (index, value) in force.iter_mut().enumerate() {
            *value += ((index % 13) as f64 - 6.0) * 0.5;
        }
        let onset_index = force.len();
        force.extend((0..360).map(|index| weight * (1.0 - 0.45 * index as f64 / 360.0)));
        force.extend((0..360).map(|index| weight * (0.55 + 1.9 * index as f64 / 360.0)));
        force.extend(std::iter::repeat_n(0.0, 400));
        let epoch = WeighingEpoch {
            start_index: 0,
            end_index: 1200,
            system_weight_newtons: weight,
            standard_deviation_newtons: 3.0,
            tied_window_count: 1,
            tied_weight_low_newtons: weight,
            tied_weight_high_newtons: weight,
        };
        (
            Trial::new(force, SAMPLE_RATE_HZ).expect("the fixture is a well formed trial"),
            epoch,
            onset_index,
        )
    }

    #[test]
    fn the_two_starts_give_different_velocities_on_the_same_trial() {
        let (trial, epoch, onset_index) = trial_and_epoch();
        let takeoff_index = 1200 + 720;
        let gravity = crate::STANDARD_GRAVITY_METERS_PER_SECOND_SQUARED;

        let from_trial_start = centre_of_mass_velocity_meters_per_second(
            &trial,
            &epoch,
            &spec(IntegrationStart::TrialStart),
            gravity,
        );
        let from_onset = centre_of_mass_velocity_meters_per_second(
            &trial,
            &epoch,
            &spec(IntegrationStart::DetectedOnset { index: onset_index }),
            gravity,
        );

        let deprecated = from_trial_start.at(takeoff_index).expect("in range");
        let recommended = from_onset.at(takeoff_index).expect("in range");
        assert!(
            (deprecated - recommended).abs() > 1e-6,
            "the deprecated and recommended starts returned the same takeoff velocity, \
             {deprecated} against {recommended}, so this trial cannot tell them apart"
        );
    }

    /// The samples the harness reads are the samples it read before the choice became
    /// stateable, so declaring the anchor does not move a frozen reference.
    #[test]
    fn the_trial_start_trapezoid_reproduces_the_running_integral_it_replaces() {
        let (trial, epoch, _) = trial_and_epoch();
        let gravity = crate::STANDARD_GRAVITY_METERS_PER_SECOND_SQUARED;
        let series = centre_of_mass_velocity_meters_per_second(
            &trial,
            &epoch,
            &spec(IntegrationStart::TrialStart),
            gravity,
        );
        let directly = cumulative_trapezoid(
            &center_of_mass_acceleration(
                trial.force(),
                epoch.system_weight_newtons,
                epoch.system_mass_kilograms(gravity),
            ),
            trial.sample_interval_seconds(),
        );
        assert_eq!(
            series.meters_per_second(),
            directly.as_slice(),
            "declaring the anchor moved the samples"
        );
    }

    #[test]
    fn a_series_carries_the_four_ids_that_produced_it() {
        let (trial, epoch, onset_index) = trial_and_epoch();
        let series = centre_of_mass_velocity_meters_per_second(
            &trial,
            &epoch,
            &spec(IntegrationStart::DetectedOnset { index: onset_index }),
            crate::STANDARD_GRAVITY_METERS_PER_SECOND_SQUARED,
        );
        assert_eq!(
            series.spec().method_ids(),
            [
                "integration.rule.trapezoid",
                "integration.direction.forward",
                "integration.start.detected_onset",
                "integration.anchor.single_point",
            ]
        );
    }

    #[test]
    fn a_two_point_anchor_holds_both_ends_at_zero() {
        let (trial, epoch, _) = trial_and_epoch();
        let mut settings = spec(IntegrationStart::TrialStart);
        settings.anchor = IntegrationAnchor::TwoPoint {
            start_index: 0,
            end_index: 1199,
        };
        let series = centre_of_mass_velocity_meters_per_second(
            &trial,
            &epoch,
            &settings,
            crate::STANDARD_GRAVITY_METERS_PER_SECOND_SQUARED,
        );
        assert!(series.at(0).expect("in range").abs() < 1e-12);
        assert!(series.at(1199).expect("in range").abs() < 1e-12);
    }

    #[test]
    fn integrating_a_constant_acceleration_matches_the_closed_form_under_every_quadrature() {
        let values = vec![2.0f64; 1201];
        for quadrature in [
            QuadratureRule::Trapezoid,
            QuadratureRule::Simpson,
            QuadratureRule::Rectangle,
        ] {
            let integrated = running_integral(&values, 0.001, quadrature);
            assert!(
                (integrated[1200] - 2.4).abs() < 1e-9,
                "{quadrature:?} integrated a constant 2.0 over 1.2 s to {}",
                integrated[1200]
            );
        }
    }

    /// The landing frame's initial value, which every other anchor pins to zero.
    #[test]
    fn an_anchor_at_a_stated_value_holds_that_value_rather_than_zero() {
        let (trial, epoch, onset_index) = trial_and_epoch();
        let stated = -2.47;
        let mut settings = spec(IntegrationStart::TrialStart);
        settings.anchor = IntegrationAnchor::SinglePointAtValue {
            index: onset_index,
            value: stated,
            stated_by_method_id: "phase.landing_end.zero_com_velocity".to_string(),
        };
        let series = centre_of_mass_velocity_meters_per_second(
            &trial,
            &epoch,
            &settings,
            crate::STANDARD_GRAVITY_METERS_PER_SECOND_SQUARED,
        );
        let at_anchor = series.at(onset_index).expect("in range");
        assert!(
            (at_anchor - stated).abs() < 1e-12,
            "the series reads {at_anchor} at the anchor where {stated} was stated"
        );
    }

    /// A value an anchor entry never stated is credited to the rule that did state it.
    #[test]
    fn an_anchor_at_a_stated_value_names_the_rule_that_stated_it() {
        let mut settings = spec(IntegrationStart::TrialStart);
        settings.anchor = IntegrationAnchor::SinglePointAtValue {
            index: 0,
            value: -2.47,
            stated_by_method_id: "phase.landing_end.zero_com_velocity".to_string(),
        };
        assert_eq!(
            settings.method_ids()[3],
            "phase.landing_end.zero_com_velocity"
        );
    }

    #[test]
    fn displacement_returns_to_the_standing_frame_it_was_anchored_in() {
        let (trial, epoch, onset_index) = trial_and_epoch();
        let velocity = centre_of_mass_velocity_meters_per_second(
            &trial,
            &epoch,
            &spec(IntegrationStart::DetectedOnset { index: onset_index }),
            crate::STANDARD_GRAVITY_METERS_PER_SECOND_SQUARED,
        );
        let displacement = centre_of_mass_displacement_meters(
            &velocity,
            &spec(IntegrationStart::DetectedOnset { index: onset_index }),
        );

        assert!(
            displacement.at(onset_index).expect("in range").abs() < 1e-12,
            "displacement is not zero where it was anchored"
        );

        // The centre of mass keeps descending for as long as velocity is negative, so
        // displacement reaches its lowest point where velocity crosses back up through zero,
        // well after velocity reached its own minimum. The two coincide only if this series
        // is the velocity rather than its integral.
        let window = onset_index..onset_index + 720;
        let lowest_of = |samples: &[f64]| {
            samples[window.clone()]
                .iter()
                .enumerate()
                .min_by(|left, right| left.1.total_cmp(right.1))
                .map(|(offset, _)| onset_index + offset)
                .expect("the window is not empty")
        };
        let slowest_index = lowest_of(velocity.meters_per_second());
        let deepest_index = lowest_of(displacement.meters());
        assert!(
            deepest_index > slowest_index + 60,
            "the centre of mass was lowest at sample {deepest_index} and slowest at \
             {slowest_index}; an integral bottoms out after its integrand does"
        );
        assert!(
            velocity.at(deepest_index).expect("in range").abs()
                < velocity.at(slowest_index).expect("in range").abs() / 4.0,
            "velocity has not returned toward zero where displacement is lowest"
        );
    }

    /// Displacement inherits every choice the velocity made, so both specs travel with it.
    /// Samples the caller wrote still arrive carrying what they are claimed to be.
    #[test]
    fn a_series_built_from_samples_still_states_its_choices() {
        let settings = spec(IntegrationStart::DetectedOnset { index: 4 });
        let series = VelocitySeries::from_samples(
            vec![0.0, -0.4, -0.9, -0.3, 0.6],
            settings.clone(),
            4,
            0.001,
        );

        assert_eq!(series.spec(), &settings);
        assert_eq!(series.first_integrated_index(), 4);
        assert_eq!(series.sample_interval_seconds(), 0.001);
        assert_eq!(
            series.spec().method_ids()[2],
            "integration.start.detected_onset"
        );
        assert_eq!(series.at(2), Some(-0.9));
    }

    #[test]
    fn displacement_carries_the_velocity_choices_it_rests_on() {
        let (trial, epoch, onset_index) = trial_and_epoch();
        let velocity_settings = spec(IntegrationStart::DetectedOnset { index: onset_index });
        let velocity = centre_of_mass_velocity_meters_per_second(
            &trial,
            &epoch,
            &velocity_settings,
            crate::STANDARD_GRAVITY_METERS_PER_SECOND_SQUARED,
        );
        let mut displacement_settings = spec(IntegrationStart::TrialStart);
        displacement_settings.quadrature = QuadratureRule::Simpson;
        let displacement = centre_of_mass_displacement_meters(&velocity, &displacement_settings);

        assert_eq!(displacement.velocity_spec(), &velocity_settings);
        assert_eq!(displacement.spec(), &displacement_settings);
        assert_eq!(
            displacement.velocity_spec().method_ids()[2],
            "integration.start.detected_onset"
        );
        assert_eq!(
            displacement.spec().method_ids()[0],
            "integration.rule.simpson"
        );
    }
}
