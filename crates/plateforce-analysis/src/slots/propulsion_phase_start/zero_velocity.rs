//! `phase.propulsion_start.zero_velocity`: velocity crosses zero from below.
//!
//! Minimum displacement and zero cumulative net impulse are the same instant, since velocity
//! is the derivative of displacement and cumulative net impulse is mass times velocity.
//!
//! The core's search returns a fallback index when velocity never returns through zero, which
//! is the behaviour of the tool it reproduces and is a different quantity under the same name.
//! This rule declines there rather than reporting it.

use plateforce_core::phases::velocity_zero_crossing;

use crate::binding::{ONSET_CONSTRUCT, TAKEOFF_CONSTRUCT};
use crate::boundaries;
use crate::derived::{DerivedContext, DerivedOutcome, DerivedRule};
use crate::request::MethodChoice;
use crate::resolution::Resolution;
use crate::response::Quantity;

pub const ID: &str = "phase.propulsion_start.zero_velocity";

pub const QUANTITIES: &[Quantity] = &[Quantity {
    key: super::KEY,
    label: "Start of propulsion",
    unit: "seconds",
    computed_by: Some(ID),
}];

pub const RULE: DerivedRule = place;

fn place(
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

    let (Some(onset), Some(takeoff)) = (context.onset_index(), context.takeoff_index()) else {
        let missing = boundaries::absent(context, &[ONSET_CONSTRUCT, TAKEOFF_CONSTRUCT]);
        return DerivedOutcome::declined(resolved.finish(), context.unavailable(ID, &missing));
    };
    let velocity = crate::centre_of_mass::velocity(
        context.trial,
        context.epoch(),
        onset,
        context.gravity_behind(None),
        &mut resolved,
    );
    let crossing = velocity_zero_crossing(&velocity, onset, takeoff);
    let bound = resolved.finish();

    boundaries::crossing_outcome(context, ID, super::KEY, super::PLACED, crossing, bound)
}
