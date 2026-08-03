//! `jumpheight.takeoff.flight_time`: gravity times flight time squared, over eight.
//!
//! The derivation assumes time up equals time down, so it holds only where the centre of mass
//! is at the same height at landing as at takeoff. Where it is not, the number measures the
//! landing frame under the takeoff frame's name: measured time down exceeds time up by
//! 0.016 s, p < 0.0001, because subjects land partially crouched.
//!
//! Which sample bounds each end is not this rule's to decide. Takeoff comes from the bound
//! takeoff rule and the return to the plate from the recording, and the entry says so.

use plateforce_core::{jump_height_from_flight_time, Refusal};

use crate::binding::{ONSET_CONSTRUCT, TAKEOFF_CONSTRUCT};
use crate::derived::{DerivedContext, DerivedOutcome, DerivedRule};
use crate::request::MethodChoice;
use crate::resolution::{Resolution, RuleRefusal};
use crate::response::Quantity;
use crate::slots::flight_time;

pub const ID: &str = "jumpheight.takeoff.flight_time";

/// The entry's own name for gravity. It publishes four values because the tools disagree, and
/// the one the request carries is the one every other number in the same result ran under.
pub const GRAVITY_PARAMETER: &str = "gravity";

pub const QUANTITIES: &[Quantity] = &[Quantity {
    key: super::FLIGHT_TIME_KEY,
    label: "Jump height, flight time",
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
    let gravity = resolved.number(GRAVITY_PARAMETER, context.gravity_meters_per_second_squared);

    let Some(landmarks) = context.landmarks() else {
        return DerivedOutcome::declined(
            resolved.finish(),
            context.unavailable(ID, &[ONSET_CONSTRUCT, TAKEOFF_CONSTRUCT]),
        );
    };
    let Some(seconds) = flight_time::seconds(context, &landmarks) else {
        return DerivedOutcome::declined(
            resolved.finish(),
            RuleRefusal::Refused(Box::new(Refusal::required_parameter_unstated(
                ID,
                flight_time::TOUCHDOWN_FIELD,
            ))),
        );
    };

    DerivedOutcome {
        values: vec![(
            super::FLIGHT_TIME_KEY,
            Some(jump_height_from_flight_time(seconds, gravity)),
        )],
        placed: Vec::new(),
        bound: resolved.finish(),
        refusal: None,
    }
}
