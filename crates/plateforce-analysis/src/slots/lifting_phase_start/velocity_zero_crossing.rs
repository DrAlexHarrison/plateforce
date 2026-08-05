//! `phase.lift.start.velocity_zero_crossing`: the declared object's velocity turns positive.
//!
//! The registry's text: the lifting phase begins when the declared object's velocity changes
//! from negative to positive, and minimum displacement and onset of positive barbell
//! displacement are the same instant.
//!
//! On a force plate the declared object is the weighed system, so this reads the same
//! centre-of-mass velocity `phase.propulsion_start.zero_velocity` reads and places the same
//! sample. That is the registry's own position rather than a coincidence: the two entries sit
//! under different constructs because they come from two literatures, the jump lineage and the
//! velocity-based-training lineage, and the registry files the pair as a homonym rather than
//! as a debate. A build that merged them would merge two literatures, so both entries stay and
//! a result says which construct it answered.
//!
//! The search runs from onset to takeoff, which on a loaded lift the athlete stays down for is
//! the whole of the recording after onset.

use plateforce_core::phases::velocity_zero_crossing;

use crate::binding::{ONSET_CONSTRUCT, TAKEOFF_CONSTRUCT};
use crate::boundaries;
use crate::derived::{DerivedContext, DerivedOutcome, DerivedRule};
use crate::request::MethodChoice;
use crate::resolution::Resolution;
use crate::response::Quantity;

pub const ID: &str = "phase.lift.start.velocity_zero_crossing";

pub const QUANTITIES: &[Quantity] = &[Quantity {
    key: super::KEY,
    label: "Start of the lift",
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
    boundaries::crossing_outcome(
        context,
        ID,
        super::KEY,
        super::PLACED,
        crossing,
        resolved.finish(),
    )
}
