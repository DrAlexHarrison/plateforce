//! `normalise.denominator`: the mass a quantity is expressed relative to, resolved to the
//! number rather than left as a word.
//!
//! One entry with three values rather than three methods, because no paper argues on principle
//! that another's denominator is wrong. What the field does instead is print a per-kilogram
//! label that does not say which kilograms, and two such numbers drawn against each other
//! differ by the athlete-to-bar mass ratio.
//!
//! The three masses are the three `declaration.computed_on_object` names, resolved by the same
//! function, so a quantity cannot be declared to describe the bar and divided by the athlete.

use crate::derived::{DerivedContext, DerivedOutcome, DerivedRule};
use crate::request::MethodChoice;
use crate::resolution::Resolution;
use crate::response::Quantity;
use crate::slots::mechanical_object::Object;

pub const ID: &str = "normalise.denominator";

/// The entry's own name for the choice, and the three values it publishes.
pub const DENOMINATOR_PARAMETER: &str = "denominator";
pub const DENOMINATORS: &[(&str, Object)] = &[
    ("body_mass", Object::Body),
    ("system_mass", Object::System),
    ("barbell_mass", Object::Barbell),
];

/// The one name this rule cannot run without.
pub const REQUIRED_OPTIONS: &[(&str, &str)] = &[(DENOMINATOR_PARAMETER, "system_mass")];

pub const QUANTITIES: &[Quantity] = &[Quantity {
    key: super::KEY,
    label: "What the number is divided by",
    unit: "kilograms",
    computed_by: Some(ID),
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
    let denominator = resolved.required_enumerated(ID, DENOMINATOR_PARAMETER, DENOMINATORS);
    let mass = denominator.and_then(|object| {
        crate::slots::mechanical_object::mass_kilograms(
            context,
            &mut resolved,
            ID,
            DENOMINATOR_PARAMETER,
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
