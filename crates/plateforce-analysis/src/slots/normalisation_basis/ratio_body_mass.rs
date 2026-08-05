//! `norm.ratio_bodymass`: the quantity per kilogram of athlete.
//!
//! Ratio scaling removes the body-size effect only if the true relationship is exactly
//! proportional, and it is not: geometric similarity predicts force scales with muscle
//! cross-sectional area, hence with mass to the two-thirds, so this over-corrects and
//! penalises heavier athletes. The exponent gap against `norm.allometric` is the registry's
//! recorded bias and the two entries file their disagreement as genuine.
//!
//! The peak it divides is the peak the caller's own peak-force rule reported, so the record
//! names that rule beside the mass. Expressing the result in multiples of bodyweight is this
//! same method with gravity folded in rather than a second one.

use crate::derived::{DerivedContext, DerivedOutcome, DerivedRule};
use crate::request::{MethodChoice, BODY_MASS_GLOBAL};
use crate::resolution::Resolution;
use crate::response::Quantity;
use crate::slots::peak_force;

pub const ID: &str = "norm.ratio_bodymass";

/// The exponent this entry is: ratio scaling is allometric scaling at one.
pub const EXPONENT: f64 = 1.0;

/// The one value this rule cannot run without, which the request binds for the whole analysis.
pub const REQUIRED_GLOBALS: &[(&str, f64)] = &[(BODY_MASS_GLOBAL, 52.0)];

pub const KEY: &str = "peak_force_per_body_mass_newtons_per_kilogram";

pub const QUANTITIES: &[Quantity] = &[Quantity {
    key: KEY,
    label: "Peak force for each kilogram of athlete",
    unit: "newtons_per_kilogram",
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
    let mass = super::body_mass_kilograms(context, &mut resolved, ID);
    let peak = super::measured(context, ID, peak_force::CONSTRUCT, peak_force::KEY);
    let bound = resolved.finish();

    let ((mass, (peak_newtons, produced_by)), ()) = match (mass, peak) {
        (Ok(mass), Ok(peak)) => ((mass, peak), ()),
        (Err(refusal), _) | (_, Err(refusal)) => return DerivedOutcome::declined(bound, refusal),
    };
    super::rests_on(context, KEY, &produced_by);
    let Some(scaled) =
        plateforce_core::normalisation::scaled_by_body_mass(peak_newtons, mass, EXPONENT)
    else {
        return DerivedOutcome::declined(bound, super::mass_not_accepted(ID, mass));
    };
    DerivedOutcome {
        values: vec![(KEY, Some(scaled))],
        placed: Vec::new(),
        bound,
        refusal: None,
    }
}
