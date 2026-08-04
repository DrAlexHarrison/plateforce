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

use crate::binding::TAKEOFF_CONSTRUCT;
use crate::derived::{DerivedContext, DerivedOutcome, DerivedRule};
use crate::request::MethodChoice;
use crate::resolution::{Resolution, RuleRefusal};
use crate::response::Quantity;
use crate::slots::flight_time;

pub const ID: &str = "jumpheight.takeoff.flight_time";

/// The entry's own name for gravity, and the value it declares.
///
/// It publishes four values because the tools disagree on this constant, and declares 9.81,
/// which is the value the paper that teaches the derivation uses. A caller who states nothing
/// gets that rather than the constant the request carries, because the request's is a struct
/// initialiser no entry declares and a record calling it assumed would be naming an act
/// nobody performed.
///
/// A caller who did choose a gravity for the analysis gets theirs, on the same reasoning read
/// the other way: half a percent of this number rides on where the plate stands, so a value
/// somebody measured there beats a value a paper published, and running the entry's constant
/// over the top of it would discard the better information in silence.
pub const GRAVITY_PARAMETER: &str = "gravity";
pub const GRAVITY_DEFAULT_METERS_PER_SECOND_SQUARED: f64 = 9.81;

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
    let gravity = resolved.number_or_chosen(
        GRAVITY_PARAMETER,
        context.chosen_gravity(),
        GRAVITY_DEFAULT_METERS_PER_SECOND_SQUARED,
    );

    // Takeoff and the return to the plate. The projectile equation reads the time off the
    // plate and the gravity, and neither of those rests on where the jump began, so this
    // reads the two samples it uses rather than the three-landmark bundle. Through the bundle
    // it declined on a recording whose onset rule found nothing, and its chain named that
    // rule and every operator it bound.
    let Some(takeoff_index) = context.takeoff_index() else {
        return DerivedOutcome::declined(
            resolved.finish(),
            context.unavailable(ID, &[TAKEOFF_CONSTRUCT]),
        );
    };
    let Some(seconds) = flight_time::seconds(context, takeoff_index) else {
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
