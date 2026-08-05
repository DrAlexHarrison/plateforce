//! `jumpheight.min_of_ft_and_tov.labanalysis`: the smaller of the flight-time height and the
//! takeoff-velocity height.
//!
//! Taking the smaller of two estimators of one quantity is not an estimator of that quantity.
//! Flight time reads above impulse-momentum on most unloaded trials and below it on loaded
//! ones, so which of the two the minimum returns changes with the load, and the number it
//! returns carries no single frame across a session.

use plateforce_core::{
    jump_height_from_flight_time, jump_height_from_takeoff_velocity,
    takeoff_velocity_meters_per_second, Refusal,
};

use crate::binding::{ONSET_CONSTRUCT, TAKEOFF_CONSTRUCT};
use crate::centre_of_mass;
use crate::derived::{DerivedContext, DerivedOutcome, DerivedRule};
use crate::request::MethodChoice;
use crate::resolution::{Resolution, RuleRefusal};
use crate::response::Quantity;
use crate::slots::flight_time;

pub const ID: &str = "jumpheight.min_of_ft_and_tov.labanalysis";

pub const QUANTITIES: &[Quantity] = &[Quantity {
    key: super::KEY,
    label: "Jump height, frame not stated",
    unit: "meters",
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
    let Some(seconds) = flight_time::seconds(context, landmarks.takeoff_index) else {
        return DerivedOutcome::declined(
            resolved.finish(),
            RuleRefusal::Refused(Box::new(Refusal::required_parameter_unstated(
                ID,
                flight_time::TOUCHDOWN_FIELD,
            ))),
        );
    };
    centre_of_mass::record_choices(&mut resolved, landmarks.onset_index);
    let bound = resolved.finish();

    let gravity = context.gravity_behind(Some(super::KEY));
    let from_flight = jump_height_from_flight_time(seconds, gravity);
    let velocity =
        takeoff_velocity_meters_per_second(context.trial, context.epoch(), &landmarks, gravity);
    let from_takeoff = jump_height_from_takeoff_velocity(velocity, gravity);

    DerivedOutcome {
        values: vec![(super::KEY, Some(from_flight.min(from_takeoff)))],
        placed: Vec::new(),
        bound,
        refusal: None,
    }
}
