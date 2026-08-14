//! `jumpheight.takeoff.flight_time`: gravity times flight time squared, over eight.
//!
//! The derivation assumes time up equals time down, so it holds only where the centre of mass
//! is at the same height at landing as at takeoff. Where it is not, the number measures the
//! landing frame under the takeoff frame's name: measured time down exceeds time up by
//! 0.016 s, p < 0.0001, because subjects land partially crouched.
//!
//! Which sample bounds each end is not this rule's to decide. Takeoff comes from the bound
//! takeoff rule and the return to the plate from the recording, and the entry says so.

use plateforce_core::jump_height_from_flight_time;

use crate::binding::TAKEOFF_CONSTRUCT;
use crate::derived::{DerivedContext, DerivedOutcome, DerivedRule};
use crate::request::MethodChoice;
use crate::resolution::Resolution;
use crate::response::Quantity;
use crate::slots::flight_time;

pub const ID: &str = "jumpheight.takeoff.flight_time";

/// The entry's own name for gravity. It publishes four values because the tools disagree on
/// this constant, and answers none of them itself.
pub const GRAVITY_PARAMETER: &str = "gravity";

pub const QUANTITIES: &[Quantity] = &[Quantity {
    key: super::FLIGHT_TIME_KEY,
    label: "Jump height, flight time",
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
    // Stated on the rule first, then a gravity somebody chose for the analysis, then the one
    // the analysis is bound to, which is what every other quantity that moves with gravity
    // runs at. The entry published its own default here, so one run carried 9.81 behind this
    // height and 9.80665 behind the other ten numbers beside it, both recorded as assumed.
    let gravity = resolved.number_or_chosen(
        GRAVITY_PARAMETER,
        context.chosen_gravity_behind(super::FLIGHT_TIME_KEY),
        context.gravity_behind(Some(super::FLIGHT_TIME_KEY)),
    );

    // Takeoff and the return to the plate. The projectile equation reads the time off the
    // plate and the gravity, and neither of those rests on where the jump began, so this
    // reads the two samples it uses rather than the three-landmark bundle.
    let Some(takeoff_index) = context.takeoff_index() else {
        return DerivedOutcome::declined(
            resolved.finish(),
            context.unavailable(ID, &[TAKEOFF_CONSTRUCT]),
        );
    };
    let Some(seconds) = flight_time::seconds(context, takeoff_index) else {
        return DerivedOutcome::declined(
            resolved.finish(),
            flight_time::no_landing_recorded(context, ID, takeoff_index),
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
