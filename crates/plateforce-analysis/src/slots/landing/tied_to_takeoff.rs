//! `landing.threshold.tied_to_takeoff`: one threshold decides both edges of the flight phase.
//!
//! The rule this software has applied since flight time was first emitted. The return to the
//! plate is searched for with the threshold the takeoff rule resolved, so a threshold error
//! compounds: a higher threshold places takeoff earlier and landing later, and flight time
//! grows at both ends rather than the two errors cancelling.
//!
//! The instant itself is the one the spine already resolved against that threshold, which is
//! why this rule reads it rather than searching again. Two searches over one trace under one
//! threshold are two implementations of one quantity, and the second would agree with the
//! first until it did not. What this adds is the name: the entry says a number produced under
//! this convention names it, and until the rule was bound the record said a landing was placed
//! by nobody.
//!
//! A caller who placed the landing by hand reaches this rule too, and `record_stated_touchdown`
//! writes that index onto the row as `Stated`, so a hand-placed landing and a found one do not
//! reach identical records.

use crate::binding::TAKEOFF_CONSTRUCT;
use crate::boundaries;
use crate::derived::{DerivedContext, DerivedOutcome, DerivedRule};
use crate::request::MethodChoice;
use crate::resolution::Resolution;
use crate::response::Quantity;

pub const ID: &str = "landing.threshold.tied_to_takeoff";

pub const QUANTITIES: &[Quantity] = &[Quantity {
    key: super::KEY,
    label: "Landing",
    unit: "seconds",
    computed_by: Some(ID),
}];

pub const RULE: DerivedRule = place;

fn place(
    context: &DerivedContext,
    choice: &MethodChoice,
    _warnings: &mut Vec<String>,
) -> DerivedOutcome {
    let resolved = Resolution::over(
        &choice.parameters,
        &choice.options,
        choice.declared.of_entry(ID),
        choice.claims(),
    );
    let bound = resolved.finish();

    // Takeoff is asked for as well as the return, because the tie is the whole of this rule:
    // the instant rests on the takeoff rule twice over, once for the threshold and once for
    // the sample the search ran from. A chain naming only the return would hide that.
    if context.takeoff_index().is_none() {
        return DerivedOutcome::declined(bound, context.unavailable(ID, &[TAKEOFF_CONSTRUCT]));
    }
    let index = context.touchdown_index();

    boundaries::placed_outcome(context, super::KEY, super::PLACED, index, bound)
}
