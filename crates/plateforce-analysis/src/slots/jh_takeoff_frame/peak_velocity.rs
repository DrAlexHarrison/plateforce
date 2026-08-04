//! `jumpheight.takeoff.peak_velocity.chavda2018`: the maximum of the velocity column, squared,
//! over twice gravity.
//!
//! Peak centre-of-mass velocity precedes takeoff, because force falls below system weight in
//! the last tens of milliseconds of propulsion while the foot is still on the plate. So this
//! reads at or above the takeoff-velocity route on every trial where the two differ, and the
//! sign of the gap is fixed by that mechanism rather than by the trace.

use plateforce_core::jump_height_from_takeoff_velocity;

use crate::binding::{ONSET_CONSTRUCT, TAKEOFF_CONSTRUCT};
use crate::centre_of_mass;
use crate::derived::{DerivedContext, DerivedOutcome, DerivedRule};
use crate::request::MethodChoice;
use crate::resolution::{Resolution, RuleRefusal};
use crate::response::Quantity;

pub const ID: &str = "jumpheight.takeoff.peak_velocity.chavda2018";

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
    let gravity = context.gravity_meters_per_second_squared;
    let series = centre_of_mass::velocity(
        context.trial,
        context.epoch(),
        landmarks.onset_index,
        gravity,
        &mut resolved,
    );
    let bound = resolved.finish();

    // The whole column, which is what the shipped worksheet takes the maximum of. Bounding it
    // at takeoff would be a different rule and would need its own entry.
    match plateforce_core::peak::maximum_over(series.meters_per_second(), 0, series.len()) {
        Ok(peak) => DerivedOutcome {
            values: vec![(
                super::KEY,
                Some(jump_height_from_takeoff_velocity(peak, gravity)),
            )],
            placed: Vec::new(),
            bound,
            refusal: None,
        },
        Err(_) => DerivedOutcome::declined(
            bound,
            RuleRefusal::Refused(Box::new(plateforce_core::Refusal::span_selects_no_samples(
                ID,
                0,
                series.len(),
            ))),
        ),
    }
}
