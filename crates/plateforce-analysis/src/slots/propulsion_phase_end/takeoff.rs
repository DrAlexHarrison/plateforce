//! `phase.propulsion_end.takeoff`: propulsion ends where the athlete leaves the plate.
//!
//! McMahon et al. 2018 states both halves of this in one paragraph: the propulsion phase
//! continues through to the instant of take-off, and peak centre of mass velocity is attained
//! before rather than at take-off, at the instant force descends through system weight. The
//! rule beside this one ends the phase at that earlier instant, so the deceleration between
//! the two is inside the phase here and outside it there.
//!
//! The instant comes from the bound takeoff rule rather than from a threshold of this rule's
//! own. Five takeoff entries place it differently, and a second threshold here would be a
//! second home for one quantity.

use crate::binding::TAKEOFF_CONSTRUCT;
use crate::boundaries;
use crate::derived::{DerivedContext, DerivedOutcome, DerivedRule};
use crate::request::MethodChoice;
use crate::resolution::Resolution;
use crate::response::Quantity;

pub const ID: &str = "phase.propulsion_end.takeoff";

pub const QUANTITIES: &[Quantity] = &[Quantity {
    key: super::KEY,
    label: "End of propulsion",
    unit: "seconds",
    computed_by: Some(ID),
}];

pub const RULE: DerivedRule = place;

fn place(
    context: &DerivedContext,
    choice: &MethodChoice,
    _warnings: &mut Vec<String>,
) -> DerivedOutcome {
    let resolved = Resolution::over(&choice.parameters, &choice.options, choice.claims());
    let bound = resolved.finish();

    let Some(takeoff) = context.takeoff_index() else {
        let missing = boundaries::absent(context, &[TAKEOFF_CONSTRUCT]);
        return DerivedOutcome::declined(bound, context.unavailable(ID, &missing));
    };
    boundaries::placed_outcome(context, super::KEY, super::PLACED, Some(takeoff), bound)
}
