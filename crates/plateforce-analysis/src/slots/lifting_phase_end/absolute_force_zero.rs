//! `phase.lift.end.absolute_force_zero.frost2008`: absolute force falls to zero.
//!
//! The registry's text: the phase ends where absolute force, with system weight not
//! subtracted, decreases to zero. It places the boundary far later than the net-force rule,
//! because absolute force reaching zero means the athlete has left the ground or released the
//! bar rather than that acceleration has ceased.
//!
//! Filed deprecated, and the entry says why that flag is soft: it deprecates another author's
//! method on a third party's testimony, the original was not obtained, and the registry asks
//! for it to be confirmed against the source before it ships. The rule runs either way. A
//! reader reproducing a published analysis needs the number the published analysis produced,
//! and refusing it would leave them with no way to get it and no record of why.
//!
//! On a jump this places the lift end at takeoff by construction, because takeoff is where
//! force reaches the plate's floor, so the deprecation is visible in a result rather than
//! only in prose.

use plateforce_core::phases::{force_reference_crossing, CrossingDirection};

use crate::binding::{ONSET_CONSTRUCT, TAKEOFF_CONSTRUCT};
use crate::boundaries;
use crate::derived::{DerivedContext, DerivedOutcome, DerivedRule};
use crate::request::MethodChoice;
use crate::resolution::Resolution;
use crate::response::Quantity;

pub const ID: &str = "phase.lift.end.absolute_force_zero.frost2008";

/// The level the entry names, which is zero because system weight is not subtracted.
const ABSOLUTE_ZERO_NEWTONS: f64 = 0.0;

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
    let resolved = Resolution::over(&choice.parameters, &choice.options, choice.claims());
    let bound = resolved.finish();

    let (Some(_onset), Some(takeoff)) = (context.onset_index(), context.takeoff_index()) else {
        let missing = boundaries::absent(context, &[ONSET_CONSTRUCT, TAKEOFF_CONSTRUCT]);
        return DerivedOutcome::declined(bound, context.unavailable(ID, &missing));
    };
    let Some(start) = super::search_start(context) else {
        return DerivedOutcome::declined(bound, context.unavailable(ID, &[ONSET_CONSTRUCT]));
    };

    // Searched to the sample takeoff was placed at rather than to the one before it. The
    // whole claim of this rule is that it fires where contact ends, so excluding that sample
    // would exclude the instant the rule exists to find.
    let crossing = force_reference_crossing(
        context.trial.force(),
        ABSOLUTE_ZERO_NEWTONS,
        start,
        takeoff + 1,
        CrossingDirection::Falling,
    );
    boundaries::crossing_outcome(context, ID, super::KEY, super::PLACED, crossing, bound)
}
