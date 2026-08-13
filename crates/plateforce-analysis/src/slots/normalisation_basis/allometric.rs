//! `norm.allometric`: the quantity per kilogram of athlete raised to a declared exponent.
//!
//! A force plate measures force, so a plate task takes the two-thirds exponent that geometric
//! similarity predicts for cross-sectional area; an exponent of one is ratio scaling, which is
//! why the ratio-against-allometric debate is real for force and vacuous for torque.
//!
//! The exponent travels with every number this reports, and the divisor is reported beside the
//! scaled value because their units differ with it. A fitted exponent is estimated from the
//! sample at hand, so two laboratories scaling allometrically with fitted exponents have not
//! computed the same thing: under that provenance the exponent has to be stated, because the
//! one this entry publishes was assumed by somebody else.

use crate::derived::{DerivedContext, DerivedOutcome, DerivedRule};
use crate::request::{MethodChoice, BODY_MASS_GLOBAL};
use crate::resolution::Resolution;
use crate::response::Quantity;
use crate::slots::peak_force;

pub const ID: &str = "norm.allometric";

/// The entry's own names, and the values it publishes.
pub const EXPONENT_PARAMETER: &str = "exponent";
pub const EXPONENT_DEFAULT: f64 = 0.67;
pub const PROVENANCE_PARAMETER: &str = "provenance";

/// Whether the exponent came from the literature or from the sample at hand.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Provenance {
    Assumed,
    Fitted,
}

pub const PROVENANCES: &[(&str, Provenance)] = &[
    ("assumed", Provenance::Assumed),
    ("fitted", Provenance::Fitted),
];

pub const REQUIRED_OPTIONS: &[(&str, &str)] = &[(PROVENANCE_PARAMETER, "assumed")];
pub const REQUIRED_GLOBALS: &[(&str, f64)] = &[(BODY_MASS_GLOBAL, 52.0)];

pub const DIVISOR_KEY: &str = "allometric_divisor_kilograms_to_the_exponent";
pub const KEY: &str = "peak_force_allometric_newtons_per_kilogram_to_the_exponent";

pub const QUANTITIES: &[Quantity] = &[
    Quantity {
        key: DIVISOR_KEY,
        label: "What the numbers are divided by",
        unit: "kilograms_to_the_exponent",
        computed_by: Some(ID),
        produced_by_construct: None,
    },
    Quantity {
        key: KEY,
        label: "Peak force scaled allometrically",
        unit: "newtons_per_kilogram_to_the_exponent",
        computed_by: Some(ID),
        produced_by_construct: None,
    },
];

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
    let provenance = resolved.required_enumerated(ID, PROVENANCE_PARAMETER, PROVENANCES);
    // Read before the branch below so the name reaches the record whichever way it goes, and
    // so a caller who did state one under a fitted provenance is credited with it.
    let stated_exponent = resolved.stated(EXPONENT_PARAMETER);
    let exponent = resolved.number(EXPONENT_PARAMETER, EXPONENT_DEFAULT);
    let mass = super::body_mass_kilograms(context, &mut resolved, ID);
    let peak = super::measured(context, ID, peak_force::CONSTRUCT, peak_force::KEY);
    let bound = resolved.finish();

    let provenance = match provenance {
        Ok(chosen) => chosen,
        Err(refusal) => return DerivedOutcome::declined(bound, refusal),
    };
    if provenance == Provenance::Fitted && stated_exponent.is_none() {
        return DerivedOutcome::declined(
            bound,
            crate::resolution::RuleRefusal::Refused(Box::new(
                plateforce_core::Refusal::required_parameter_unstated(ID, EXPONENT_PARAMETER),
            )),
        );
    }
    let (mass, (peak_newtons, produced_by)) = match (mass, peak) {
        (Ok(mass), Ok(peak)) => (mass, peak),
        (Err(refusal), _) | (_, Err(refusal)) => return DerivedOutcome::declined(bound, refusal),
    };
    let (Some(divisor), Some(scaled)) = (
        plateforce_core::normalisation::body_mass_divisor(mass, exponent),
        plateforce_core::normalisation::scaled_by_body_mass(peak_newtons, mass, exponent),
    ) else {
        return DerivedOutcome::declined(bound, super::mass_not_accepted(ID, mass));
    };
    super::rests_on(context, KEY, &produced_by);
    super::rests_on(context, DIVISOR_KEY, &produced_by);
    DerivedOutcome {
        values: vec![(DIVISOR_KEY, Some(divisor)), (KEY, Some(scaled))],
        placed: Vec::new(),
        bound,
        refusal: None,
    }
}
