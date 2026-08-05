//! `phase.lift.end.net_force_zero`: net force on the declared object falls through zero.
//!
//! The registry's text: the phase ends when measured force minus the weight of the declared
//! object crosses zero from positive to negative, and that instant is analytically identical
//! to peak velocity of the object. Two schools derived it from opposite directions, and at
//! one-repetition maximum it coincides with the peak-displacement rule because there is no
//! braking phase left to disagree about.
//!
//! The deadband is a parameter rather than a method: on a smooth trace near a zero crossing,
//! 10 N is a few milliseconds. It widens the level the trace has to fall through, so a stated
//! deadband places the boundary later than a bare zero and never earlier.
//!
//! The entry's own status is the one place the loader and the prose disagree, and the loader
//! is right: the paper that proposes the rule and carries the argument for it is behind a
//! publisher that returned nothing to any probe.

use plateforce_core::phases::{force_reference_crossing, CrossingDirection};

use crate::binding::{ONSET_CONSTRUCT, TAKEOFF_CONSTRUCT};
use crate::boundaries;
use crate::derived::{DerivedContext, DerivedOutcome, DerivedRule};
use crate::request::MethodChoice;
use crate::resolution::Resolution;
use crate::response::Quantity;

pub const ID: &str = "phase.lift.end.net_force_zero";

/// The entry's own name for the tolerance, and the value it publishes as its default.
pub const DEADBAND_PARAMETER: &str = "deadband_n";
const DEADBAND_DEFAULT_NEWTONS: f64 = 0.0;

pub const QUANTITIES: &[Quantity] = &[Quantity {
    key: super::KEY,
    label: "End of the lift",
    unit: "seconds",
    computed_by: Some(ID),
}];

pub const RULE: DerivedRule = place;

fn place(
    context: &DerivedContext,
    choice: &MethodChoice,
    _warnings: &mut Vec<String>,
) -> DerivedOutcome {
    let mut resolved = Resolution::over(&choice.parameters, &choice.options, choice.claims());
    let deadband_newtons = resolved.number(DEADBAND_PARAMETER, DEADBAND_DEFAULT_NEWTONS);

    let (Some(_onset), Some(takeoff)) = (context.onset_index(), context.takeoff_index()) else {
        let missing = boundaries::absent(context, &[ONSET_CONSTRUCT, TAKEOFF_CONSTRUCT]);
        return DerivedOutcome::declined(resolved.finish(), context.unavailable(ID, &missing));
    };
    let Some(start) = super::search_start(context) else {
        return DerivedOutcome::declined(
            resolved.finish(),
            context.unavailable(ID, &[ONSET_CONSTRUCT]),
        );
    };

    // The declared object on a plate-only recording is the weighed system, so net force is
    // measured force less system weight and the crossing is of system weight itself. The
    // deadband lowers the level, which is what makes it a tolerance rather than a rule.
    let reference_newtons = context.epoch().system_weight_newtons - deadband_newtons;
    let crossing = force_reference_crossing(
        context.trial.force(),
        reference_newtons,
        start,
        takeoff,
        CrossingDirection::Falling,
    );
    boundaries::crossing_outcome(
        context,
        ID,
        super::KEY,
        super::PLACED,
        crossing,
        resolved.finish(),
    )
}
