//! `jumpheight.takeoff.impulse_momentum`: takeoff velocity squared over twice gravity.
//!
//! Takeoff velocity is the net impulse over system mass, which is an identity rather than an
//! estimate, so this is the reference the other three are quoted against. Three further
//! registry entries sit inside it and each is its own row: the weighing epoch, the
//! integration start, and the takeoff threshold. It reads all three off the spine.

use plateforce_core::{jump_height_from_takeoff_velocity, takeoff_velocity_meters_per_second};

use crate::binding::{ONSET_CONSTRUCT, TAKEOFF_CONSTRUCT};
use crate::centre_of_mass;
use crate::derived::{DerivedContext, DerivedOutcome, DerivedRule};
use crate::request::MethodChoice;
use crate::resolution::Resolution;
use crate::response::Quantity;

pub const ID: &str = "jumpheight.takeoff.impulse_momentum";

pub const QUANTITIES: &[Quantity] = &[Quantity {
    key: super::KEY,
    label: "Jump height, takeoff frame",
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
    // The velocity this rests on is read off the integrated series, so the four entries behind
    // that series are behind this number. They place no sample, so nothing else in the record
    // of what this rule read can reach them.
    let spec = centre_of_mass::spec_anchored_at(landmarks.onset_index);
    context.rests_on(super::KEY, &spec.method_ids());
    centre_of_mass::record_choices(&mut resolved, landmarks.onset_index);
    let bound = resolved.finish();

    let gravity = context.gravity_behind(Some(super::KEY));
    let velocity =
        takeoff_velocity_meters_per_second(context.trial, context.epoch(), &landmarks, gravity);
    DerivedOutcome {
        values: vec![(
            super::KEY,
            Some(jump_height_from_takeoff_velocity(velocity, gravity)),
        )],
        placed: Vec::new(),
        bound,
        refusal: None,
    }
}
