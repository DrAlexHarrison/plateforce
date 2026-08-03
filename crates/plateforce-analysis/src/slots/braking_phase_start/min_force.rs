//! `phase.braking_start.min_force`: the force nadir following onset.
//!
//! A legitimate boundary in one published phase model and a name collision with the other.
//! It strictly precedes the net-force crossing, so a user importing two products' braking
//! columns is reading two different measurements under one heading.
//!
//! The search is bounded at the propulsive peak. Bounded at takeoff it returns the sample
//! before takeoff, because force is still collapsing toward zero there and never turns back
//! up: one shipped tool computes the boundary both ways and overwrites the first with the
//! second, and only the second reaches its output.

use plateforce_core::phases::braking_start_by_force_minimum;

use crate::binding::{ONSET_CONSTRUCT, TAKEOFF_CONSTRUCT};
use crate::boundaries;
use crate::derived::{DerivedContext, DerivedOutcome, DerivedRule};
use crate::request::MethodChoice;
use crate::resolution::Resolution;
use crate::response::Quantity;

pub const ID: &str = "phase.braking_start.min_force";

pub const QUANTITIES: &[Quantity] = &[Quantity {
    key: super::KEY,
    label: "Start of braking",
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
        &choice.recommended,
        &choice.from_registry_default,
    );
    let bound = resolved.finish();

    let (Some(onset), Some(takeoff)) = (context.onset_index, context.takeoff_index) else {
        let missing = boundaries::absent(context, &[ONSET_CONSTRUCT, TAKEOFF_CONSTRUCT]);
        return DerivedOutcome::declined(bound, context.unavailable(ID, &missing));
    };

    let placed = boundaries::propulsive_peak_index(context, onset, takeoff)
        .and_then(|peak| braking_start_by_force_minimum(context.trial.force(), onset, peak));

    let Some(index) = placed else {
        return DerivedOutcome {
            values: vec![(super::KEY, None)],
            placed: Vec::new(),
            bound,
            refusal: None,
        };
    };
    DerivedOutcome {
        values: vec![(super::KEY, Some(context.trial.time_at(index)))],
        placed: vec![(super::PLACED, index)],
        bound,
        refusal: None,
    }
}
