//! `declaration.computed_on_object`: which object every force, velocity, power, work and
//! impulse in a loaded lift describes.
//!
//! The subtrahend is not a constant of the analysis, it is a function of which object is being
//! described. Subtracting system weight from a barbell-derived force, and barbell weight from
//! a plate-derived force, are both errors and both easy to make. The registry's verdict is that
//! this is the first question asked whenever a load is declared, so nothing is defaulted.
//!
//! What is deprecated is not any one object but using one object's kinematics to claim
//! another's quantity.

use crate::derived::{DerivedContext, DerivedOutcome, DerivedRule};
use crate::request::MethodChoice;
use crate::resolution::Resolution;
use crate::response::Quantity;

use super::Object;

pub const ID: &str = "declaration.computed_on_object";

/// The entry's own name for the choice, and the three values it publishes.
pub const OBJECT_PARAMETER: &str = "object";
pub const OBJECTS: &[(&str, Object)] = &[
    ("barbell", Object::Barbell),
    ("body", Object::Body),
    ("system", Object::System),
];

/// The one name this rule cannot run without.
pub const REQUIRED_OPTIONS: &[(&str, &str)] = &[(OBJECT_PARAMETER, "system")];

pub const QUANTITIES: &[Quantity] = &[Quantity {
    key: super::KEY,
    label: "Mass of the object the numbers describe",
    unit: "kilograms",
    computed_by: Some(ID),
    produced_by_construct: None,
}];

pub const RULE: DerivedRule = compute;

fn compute(
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
    let object = resolved.required_enumerated(ID, OBJECT_PARAMETER, OBJECTS);
    let mass = object.and_then(|object| {
        super::mass_kilograms(
            context,
            &mut resolved,
            ID,
            OBJECT_PARAMETER,
            object,
            super::KEY,
        )
    });
    let bound = resolved.finish();

    match mass {
        Ok(kilograms) => DerivedOutcome {
            values: vec![(super::KEY, Some(kilograms))],
            placed: Vec::new(),
            bound,
            refusal: None,
        },
        Err(refusal) => DerivedOutcome::declined(bound, refusal),
    }
}
