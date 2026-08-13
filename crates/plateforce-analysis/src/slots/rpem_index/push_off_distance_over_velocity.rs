//! `index.rpem.riosgallardo`: jump height over a push-off duration of distance over velocity.
//!
//! Which velocity divides the distance is the whole of this rule, and the entry stated no
//! answer. The source's own data settles it. Its analysis reads a `Velocity (m/s)` column out
//! of a vendor export and never says what the column holds; over the 2857 rows of that export
//! carrying a positive height, distance and velocity, the column is the takeoff velocity the
//! height implies divided by two, at a median ratio of 0.5000 and a standard deviation of
//! 0.0012. That is the mean velocity of a push-off whose acceleration is uniform, which is the
//! model the vendor's push-off distance comes from.
//!
//! So the duration this rule divides by is a modelled one and not a measured interval, and the
//! difference between the two is the difference between an assumed uniform acceleration and
//! the one the plate recorded. A plate can supply either, and it can supply the reading the
//! word "velocity" invites, so the choice is stated rather than assumed and each of the three
//! is a different number from the same recording.
//!
//! The source reports the index in centimetres per millisecond. The construct is metres per
//! second, and one of the former is ten of the latter.

use plateforce_core::{
    jump_height_from_takeoff_velocity, reactive_strength_index_modified,
    takeoff_velocity_meters_per_second, Refusal,
};

use crate::binding::{ONSET_CONSTRUCT, TAKEOFF_CONSTRUCT};
use crate::centre_of_mass;
use crate::derived::{DerivedContext, DerivedOutcome, DerivedRule};
use crate::request::MethodChoice;
use crate::resolution::{Resolution, RuleRefusal};
use crate::response::Quantity;
use crate::slots::propulsion_phase_start;

pub const ID: &str = "index.rpem.riosgallardo";

/// Which velocity the push-off distance is divided by.
pub const VELOCITY_TERM_PARAMETER: &str = "velocity_term";

/// Half the takeoff velocity, the mean of a velocity rising linearly from rest. What the
/// source's own export holds.
pub const VELOCITY_MEAN_MODELLED: &str = "mean_from_takeoff_velocity";
/// The distance divided by the interval the plate recorded it over, which is the mean velocity
/// of the push-off as it happened rather than as the model implies.
pub const VELOCITY_MEAN_MEASURED: &str = "mean_over_push_off";
/// The instantaneous velocity at takeoff, which is the reading the bare word invites and is
/// twice the source's.
pub const VELOCITY_AT_TAKEOFF: &str = "takeoff";

/// Which velocity a probe of this rule states, for the checks that sweep every parameter a
/// rule needs before it will run. The entry publishes no default, so a rule reached without
/// one declines at every value and reads as a control that moves nothing.
pub const REQUIRED_OPTIONS: &[(&str, &str)] = &[(VELOCITY_TERM_PARAMETER, VELOCITY_MEAN_MODELLED)];

/// Which velocity each stated name selects. The value is the index into the three below rather
/// than a closure, so the accepted set and the arithmetic cannot drift apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VelocityTerm {
    MeanModelled,
    MeanMeasured,
    AtTakeoff,
}

const VELOCITY_TERMS: &[(&str, VelocityTerm)] = &[
    (VELOCITY_MEAN_MODELLED, VelocityTerm::MeanModelled),
    (VELOCITY_MEAN_MEASURED, VelocityTerm::MeanMeasured),
    (VELOCITY_AT_TAKEOFF, VelocityTerm::AtTakeoff),
];

pub const QUANTITIES: &[Quantity] = &[Quantity {
    key: super::KEY,
    label: "RPeM index",
    unit: "meters_per_second",
    computed_by: Some(ID),
    produced_by_construct: None,
}];

pub const RULE: DerivedRule = compute;

fn compute(
    context: &DerivedContext,
    choice: &MethodChoice,
    _warnings: &mut Vec<String>,
) -> DerivedOutcome {
    let mut resolved = Resolution::over(
        &choice.parameters,
        &choice.options,
        choice.declared.of_entry(ID),
        choice.claims(),
    );
    let term = match resolved.required_enumerated(ID, VELOCITY_TERM_PARAMETER, VELOCITY_TERMS) {
        Ok(term) => term,
        Err(refusal) => return DerivedOutcome::declined(resolved.finish(), refusal),
    };

    let Some(landmarks) = context.landmarks() else {
        return DerivedOutcome::declined(
            resolved.finish(),
            context.unavailable(ID, &[ONSET_CONSTRUCT, TAKEOFF_CONSTRUCT]),
        );
    };
    let Some(push_off_start) = propulsion_phase_start::placed(context) else {
        return DerivedOutcome::declined(
            resolved.finish(),
            context.unavailable(ID, &[propulsion_phase_start::CONSTRUCT]),
        );
    };

    // The height, the takeoff velocity and the push-off distance all come off one integrated
    // series, so the four integration entries behind that series are behind this number too
    // and the ratio cannot be assembled from two series that agree only approximately.
    let spec = centre_of_mass::spec_anchored_at(landmarks.onset_index);
    context.rests_on(super::KEY, &spec.method_ids());

    let gravity = context.gravity_behind(Some(super::KEY));
    let displacement = centre_of_mass::displacement(
        context.trial,
        context.epoch(),
        landmarks.onset_index,
        gravity,
        &mut resolved,
    );
    let last_in_contact = centre_of_mass::last_sample_in_contact(landmarks.takeoff_index);
    let meters = displacement.meters();
    if push_off_start >= last_in_contact || last_in_contact >= meters.len() {
        return DerivedOutcome::declined(
            resolved.finish(),
            RuleRefusal::Refused(Box::new(Refusal::span_selects_no_samples(
                ID,
                push_off_start,
                last_in_contact,
            ))),
        );
    }
    let push_off_distance_meters = meters[last_in_contact] - meters[push_off_start];

    let takeoff_velocity =
        takeoff_velocity_meters_per_second(context.trial, context.epoch(), &landmarks, gravity);
    let interval_seconds = context.trial.sample_interval_seconds();
    let measured_push_off_seconds =
        (last_in_contact as f64 - push_off_start as f64) * interval_seconds;
    let velocity_meters_per_second = match term {
        VelocityTerm::MeanModelled => takeoff_velocity / 2.0,
        VelocityTerm::MeanMeasured => push_off_distance_meters / measured_push_off_seconds,
        VelocityTerm::AtTakeoff => takeoff_velocity,
    };

    // A push-off that descends, or a velocity at or below zero, describes an athlete who was
    // still going down at the instant the takeoff rule placed. Dividing anyway returns a
    // negative duration and then a negative index, which reads as a small number rather than
    // as a rule that met a recording it cannot describe.
    if push_off_distance_meters <= 0.0 || velocity_meters_per_second <= 0.0 {
        return DerivedOutcome::declined(
            resolved.finish(),
            RuleRefusal::Refused(Box::new(Refusal::nothing_qualified(
                ID,
                1,
                std::collections::BTreeMap::from([
                    (
                        "push_off_distance_meters".to_string(),
                        push_off_distance_meters,
                    ),
                    (
                        "velocity_meters_per_second".to_string(),
                        velocity_meters_per_second,
                    ),
                ]),
            ))),
        );
    }
    let derived_push_off_seconds = push_off_distance_meters / velocity_meters_per_second;
    let height_meters = jump_height_from_takeoff_velocity(takeoff_velocity, gravity);

    // The family's one division, with its guard against a non-positive denominator. What this
    // rule hands it is the derived push-off duration rather than the time to takeoff, which is
    // the whole difference between this entry and the two that share that function's name.
    DerivedOutcome {
        values: vec![(
            super::KEY,
            reactive_strength_index_modified(height_meters, derived_push_off_seconds),
        )],
        placed: Vec::new(),
        bound: resolved.finish(),
        refusal: None,
    }
}
