//! `jumpheight.standing.double_integration`: the highest the displacement curve reaches
//! between quiet standing and the return to the plate.
//!
//! One integral covers both halves of the jump, so the apex needs no separate flight term: the
//! plate reads nothing during flight, the net force is one system weight downward, and the
//! second integral carries the centre of mass up and back down under gravity on its own.

use plateforce_core::Refusal;

use crate::binding::{ONSET_CONSTRUCT, TAKEOFF_CONSTRUCT};
use crate::centre_of_mass;
use crate::derived::{DerivedContext, DerivedOutcome, DerivedRule};
use crate::request::MethodChoice;
use crate::resolution::{Resolution, RuleRefusal};
use crate::response::Quantity;
use crate::slots::flight_time;

pub const ID: &str = "jumpheight.standing.double_integration";

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
    // The search ends where the athlete came back down. Run past that and the curve is an
    // integral of a landing, and run to the end of an untrimmed recording and it is an integral
    // of whatever the athlete did next.
    let Some(touchdown_index) = context.touchdown_index() else {
        return DerivedOutcome::declined(
            resolved.finish(),
            flight_time::no_landing_recorded(context, ID, landmarks.takeoff_index),
        );
    };

    let displacement = centre_of_mass::displacement(
        context.trial,
        context.epoch(),
        landmarks.onset_index,
        context.gravity_behind(Some(super::KEY)),
        &mut resolved,
    );
    let bound = resolved.finish();

    let end = touchdown_index.saturating_add(1).min(displacement.len());
    match plateforce_core::peak::maximum_over(displacement.meters(), landmarks.onset_index, end) {
        Ok(apex_meters) => DerivedOutcome {
            values: vec![(super::KEY, Some(apex_meters))],
            placed: Vec::new(),
            bound,
            refusal: None,
        },
        Err(_) => DerivedOutcome::declined(
            bound,
            RuleRefusal::Refused(Box::new(Refusal::span_selects_no_samples(
                ID,
                landmarks.onset_index,
                end,
            ))),
        ),
    }
}
