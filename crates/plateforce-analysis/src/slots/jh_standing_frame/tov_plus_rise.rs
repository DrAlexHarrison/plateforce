//! `jumpheight.standing.tov_plus_displacement`: the rise to takeoff, plus the rise after it.
//!
//! Two terms measuring two different stretches of the same jump. The first is the centre of
//! mass climbing from quiet standing to the instant the foot leaves, read off the displacement
//! curve. The second is the flight, from takeoff velocity. Their sum is the apex above
//! standing, and dropping the first term is what makes a takeoff-frame number.

use plateforce_core::{jump_height_from_takeoff_velocity, takeoff_velocity_meters_per_second};

use crate::binding::{ONSET_CONSTRUCT, TAKEOFF_CONSTRUCT};
use crate::centre_of_mass;
use crate::derived::{DerivedContext, DerivedOutcome, DerivedRule};
use crate::request::MethodChoice;
use crate::resolution::{Resolution, RuleRefusal};
use crate::response::Quantity;

pub const ID: &str = "jumpheight.standing.tov_plus_displacement";

pub const QUANTITIES: &[Quantity] = &[Quantity {
    key: super::KEY,
    label: "Jump height, standing frame",
    unit: "meters",
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

    let Some(landmarks) = context.landmarks() else {
        return DerivedOutcome::declined(
            resolved.finish(),
            context.unavailable(ID, &[ONSET_CONSTRUCT, TAKEOFF_CONSTRUCT]),
        );
    };
    let gravity = context.gravity_behind(Some(super::KEY));
    let displacement = centre_of_mass::displacement(
        context.trial,
        context.epoch(),
        landmarks.onset_index,
        gravity,
        &mut resolved,
    );
    let bound = resolved.finish();

    let at_takeoff = centre_of_mass::last_sample_in_contact(landmarks.takeoff_index);
    let Some(rise_to_takeoff_meters) = displacement.at(at_takeoff) else {
        return DerivedOutcome::declined(
            bound,
            RuleRefusal::Refused(Box::new(plateforce_core::Refusal::span_selects_no_samples(
                ID,
                landmarks.onset_index,
                at_takeoff,
            ))),
        );
    };

    let velocity =
        takeoff_velocity_meters_per_second(context.trial, context.epoch(), &landmarks, gravity);
    DerivedOutcome {
        values: vec![(
            super::KEY,
            Some(rise_to_takeoff_meters + jump_height_from_takeoff_velocity(velocity, gravity)),
        )],
        placed: Vec::new(),
        bound,
        refusal: None,
    }
}
