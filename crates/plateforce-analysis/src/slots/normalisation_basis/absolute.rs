//! `norm.absolute`: the measured quantity, unmodified.
//!
//! The basis that divides by one, and the field's own consensus review declines to endorse any
//! scaling function over it. Registered and reported rather than left implicit, because a
//! registry that lets a result carry a scaled value with no record of the basis has destroyed
//! the information a reader needs to draw it against anybody else's.

use crate::derived::{DerivedContext, DerivedOutcome, DerivedRule};
use crate::request::MethodChoice;
use crate::resolution::Resolution;
use crate::response::Quantity;

pub const ID: &str = "norm.absolute";

pub const KEY: &str = "normalisation_divisor_dimensionless";

pub const QUANTITIES: &[Quantity] = &[Quantity {
    key: KEY,
    label: "What the numbers are divided by",
    unit: "dimensionless",
    computed_by: Some(ID),
    produced_by_construct: None,
}];

pub const RULE: DerivedRule = compute;

fn compute(
    _context: &DerivedContext,
    choice: &MethodChoice,
    _warnings: &mut Vec<String>,
) -> DerivedOutcome {
    let resolved = Resolution::over(
        &choice.parameters,
        &choice.options,
        choice.declared.of_entry(ID),
        choice.claims(),
    );
    DerivedOutcome {
        values: vec![(KEY, Some(1.0))],
        placed: Vec::new(),
        bound: resolved.finish(),
        refusal: None,
    }
}
