//! `rsimod.jh_tov_over_ttt`: the takeoff-velocity jump height, over the time from onset to
//! takeoff.
//!
//! The registry files a second numerator, `rsimod.jh_ft_over_ttt`, which divides the
//! flight-time height by the same interval and is a different number on the same recording.
//! Which of the two produced a figure is what this entry id says, so a result carrying it
//! answers the question the shared name invites.
//!
//! The fragility is measured and it is in the denominator: typical error at 7.5 to 9.3 percent
//! against 2 to 3 percent for the height alone, in two labs independently.

use plateforce_core::{
    jump_height_from_takeoff_velocity, reactive_strength_index_modified,
    takeoff_velocity_meters_per_second, time_to_takeoff_seconds,
};

use crate::binding::{ONSET_CONSTRUCT, TAKEOFF_CONSTRUCT};
use crate::centre_of_mass;
use crate::derived::{DerivedContext, DerivedOutcome, DerivedRule};
use crate::request::MethodChoice;
use crate::resolution::Resolution;
use crate::response::Quantity;

pub const ID: &str = "rsimod.jh_tov_over_ttt";

pub const QUANTITIES: &[Quantity] = &[Quantity {
    key: super::KEY,
    label: "RSI modified",
    unit: "meters_per_second",
    computed_by: Some(ID),
}];

pub const RULE: DerivedRule = compute;

fn compute(
    context: &DerivedContext,
    choice: &MethodChoice,
    _warnings: &mut Vec<String>,
) -> DerivedOutcome {
    let mut resolved = Resolution::over(&choice.parameters, &choice.options, choice.claims());

    let Some(landmarks) = context.landmarks() else {
        return DerivedOutcome::declined(
            resolved.finish(),
            context.unavailable(ID, &[ONSET_CONSTRUCT, TAKEOFF_CONSTRUCT]),
        );
    };

    // The numerator is read off the integrated velocity series, so the four entries behind
    // that series are behind this number too.
    let spec = centre_of_mass::spec_anchored_at(landmarks.onset_index);
    context.rests_on(super::KEY, &spec.method_ids());
    centre_of_mass::record_choices(&mut resolved, landmarks.onset_index);

    let gravity = context.gravity_behind(Some(super::KEY));
    let velocity =
        takeoff_velocity_meters_per_second(context.trial, context.epoch(), &landmarks, gravity);
    let height_meters = jump_height_from_takeoff_velocity(velocity, gravity);
    let seconds = time_to_takeoff_seconds(&landmarks, context.trial.sample_interval_seconds());

    DerivedOutcome {
        values: vec![(
            super::KEY,
            reactive_strength_index_modified(height_meters, seconds),
        )],
        placed: Vec::new(),
        bound: resolved.finish(),
        refusal: None,
    }
}
