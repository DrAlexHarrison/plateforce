//! `phase.lift.end.peak_displacement.lake2012_PD`: the lift ends at the top of the movement.
//!
//! The registry's text: the phase ends when the tracked object reaches maximum vertical
//! displacement, which includes the braking sub-phase. That is the whole disagreement with
//! the net-force rule: the interval between the two is the stretch over which the lifter is
//! decelerating the load, and it is inside the phase here and outside it there.
//!
//! The search stops at the last sample in contact. Past takeoff the plate measures nothing
//! about the object, and the reconstructed centre of mass keeps rising to the apex of the
//! flight, so an unbounded search on a jump would report the top of the jump under a rule
//! about the top of a lift.
//!
//! On an unloaded ballistic jump displacement rises monotonically from the zero-velocity
//! instant to takeoff, so this rule places the boundary at the last sample in contact and the
//! distinction from the other two rules vanishes. The registry says the same thing in words:
//! the athlete leaves the ground rather than decelerating the load. It is measurable on this
//! build rather than argued.

use crate::binding::{ONSET_CONSTRUCT, TAKEOFF_CONSTRUCT};
use crate::boundaries;
use crate::derived::{DerivedContext, DerivedOutcome, DerivedRule};
use crate::request::MethodChoice;
use crate::resolution::Resolution;
use crate::response::Quantity;

pub const ID: &str = "phase.lift.end.peak_displacement.lake2012_PD";

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
    let Some(start) = super::search_start(context) else {
        return DerivedOutcome::declined(
            resolved.finish(),
            context.unavailable(ID, &[ONSET_CONSTRUCT]),
        );
    };

    // Displacement is the velocity integrated a second time, and reading it through the one
    // helper keeps the four integration choices behind this boundary recorded rather than
    // silently inherited.
    let displacement = crate::centre_of_mass::displacement(
        context.trial,
        context.epoch(),
        onset,
        context.gravity_behind(None),
        &mut resolved,
    );
    let last_in_contact = crate::centre_of_mass::last_sample_in_contact(takeoff);
    let index = plateforce_core::peak::index_of_maximum_over(
        displacement.meters(),
        start,
        last_in_contact + 1,
    )
    .ok();
    boundaries::placed_outcome(context, super::KEY, super::PLACED, index, resolved.finish())
}
