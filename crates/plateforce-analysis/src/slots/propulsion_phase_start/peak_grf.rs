//! `phase.propulsion_start.peak_grf`: the instant of peak vertical force.
//!
//! Deprecated, and carried so an analysis published under it can be reproduced and labelled.
//! Peak force is not the transition point and there is no mechanical reason it should be: in
//! a bimodal-secondary profile the global force maximum occurs in the upward phase entirely,
//! so the boundary can land on the wrong side of the transition.
//!
//! The entry names two instants, the maximum or the sample just prior to it, and states no
//! rule for choosing between them. The maximum is taken, and the entry's own bias figure
//! against the velocity crossing is 0.05 s, against which one sample is not the disagreement
//! this rule is carried to show.

use crate::binding::{ONSET_CONSTRUCT, TAKEOFF_CONSTRUCT};
use crate::boundaries;
use crate::derived::{DerivedContext, DerivedOutcome, DerivedRule};
use crate::request::MethodChoice;
use crate::resolution::Resolution;
use crate::response::Quantity;

pub const ID: &str = "phase.propulsion_start.peak_grf";

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
    let resolved = Resolution::over(
        &choice.parameters,
        &choice.options,
        choice.declared.of_entry(ID),
        choice.claims(),
    );
    let bound = resolved.finish();

    let (Some(onset), Some(takeoff)) = (context.onset_index(), context.takeoff_index()) else {
        let missing = boundaries::absent(context, &[ONSET_CONSTRUCT, TAKEOFF_CONSTRUCT]);
        return DerivedOutcome::declined(bound, context.unavailable(ID, &missing));
    };

    // The jump, meaning onset to takeoff. Over an untrimmed recording the largest force is
    // the landing, and this rule would place the start of propulsion after the athlete had
    // already come back down.
    let index = boundaries::propulsive_peak_index(context, onset, takeoff);
    boundaries::placed_outcome(context, super::KEY, super::PLACED, index, bound)
}
