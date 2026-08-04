//! `jumpheight.dj.box_height_as_drop_height`: the box height stands in for the arrival.
//!
//! A drop jump starts with the athlete already moving downward, which the impulse-momentum
//! integration cannot know, because it starts from rest. This rule supplies the lower boundary
//! condition by treating the box the athlete stepped off as the height their centre of mass
//! fell through, so takeoff velocity is the integrated change plus that arrival velocity.
//!
//! The substitution is why the entry is deprecated and shipped anyway. The centre of mass
//! falls less than the box is tall, because the athlete steps down rather than dropping
//! rigidly, and the registry records 0.066 m of disagreement against the two-plate rule that
//! measures the arrival instead of assuming it. Commercial software commonly implements
//! exactly this, so a reader who wants to reproduce a number their vendor gave them needs it.

use plateforce_core::{
    drop_touchdown_velocity_meters_per_second, jump_height_from_takeoff_velocity,
    takeoff_velocity_meters_per_second, Refusal,
};

use crate::binding::{ONSET_CONSTRUCT, TAKEOFF_CONSTRUCT};
use crate::centre_of_mass;
use crate::derived::{DerivedContext, DerivedOutcome, DerivedRule};
use crate::request::MethodChoice;
use crate::resolution::{Resolution, RuleRefusal};
use crate::response::Quantity;

pub const ID: &str = "jumpheight.dj.box_height_as_drop_height";

/// The height the athlete stepped off. Required, and no value is filled in for it: there is no
/// representative box, and one study assumed a single box height for all 24 of its subjects.
pub const BOX_HEIGHT_PARAMETER: &str = "box_height_m";

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
    let box_height_meters = match resolved.required_number(ID, BOX_HEIGHT_PARAMETER) {
        Ok(value) => value,
        Err(refusal) => return DerivedOutcome::declined(resolved.finish(), refusal),
    };

    let Some(landmarks) = context.landmarks() else {
        return DerivedOutcome::declined(
            resolved.finish(),
            context.unavailable(ID, &[ONSET_CONSTRUCT, TAKEOFF_CONSTRUCT]),
        );
    };
    // The integrated change this adds to is read off the same series the impulse-momentum rule
    // reads, so the four integration entries behind that series are behind this number too.
    let spec = centre_of_mass::spec_anchored_at(landmarks.onset_index);
    context.rests_on(super::KEY, &spec.method_ids());
    centre_of_mass::record_choices(&mut resolved, landmarks.onset_index);
    let bound = resolved.finish();

    let gravity = context.gravity_meters_per_second_squared;
    let change_from_rest =
        takeoff_velocity_meters_per_second(context.trial, context.epoch(), &landmarks, gravity);
    let arrival = drop_touchdown_velocity_meters_per_second(box_height_meters, gravity);
    let takeoff_velocity = change_from_rest + arrival;

    // A takeoff velocity at or below zero says the athlete was still descending at the instant
    // the takeoff rule placed, which no jump does. Squaring it would report the descent as a
    // height, and the further the box height is from the truth the smaller that height looks,
    // so the reader would read a number that gets quieter as the input gets worse. The stated
    // box is the thing that is wrong, and the refusal names it.
    if takeoff_velocity <= 0.0 {
        return DerivedOutcome::declined(
            bound,
            RuleRefusal::Refused(Box::new(Refusal::value_not_accepted(
                ID,
                BOX_HEIGHT_PARAMETER,
                box_height_meters,
                vec![format!(
                    "a drop height below {:.4} m, above which the arrival it implies exceeds the \
                     {change_from_rest:.4} m/s this contact phase produced",
                    change_from_rest.powi(2) / (2.0 * gravity)
                )],
            ))),
        );
    }

    DerivedOutcome {
        values: vec![(
            super::KEY,
            Some(jump_height_from_takeoff_velocity(takeoff_velocity, gravity)),
        )],
        placed: Vec::new(),
        bound,
        refusal: None,
    }
}
