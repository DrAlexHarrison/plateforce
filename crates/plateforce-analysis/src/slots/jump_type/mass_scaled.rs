//! `qc.jump_type_autodetection.mass_scaled`: the same classification with the threshold
//! scaled by the athlete's mass against an anchor.
//!
//! Force capacity scales with muscle cross-sectional area, which under geometric similarity
//! goes as mass to the two-thirds, and that is the exponent the allometric normalisation
//! entry already carries. An exponent of zero recovers the fixed threshold exactly and an
//! exponent of one makes it a constant fraction of bodyweight, so the two published endpoints
//! are values of one parameter rather than a third position invented between them.
//!
//! The athlete's mass is not the weighed system mass and the rule declines by name rather
//! than dividing by the other one.

use crate::derived::{DerivedContext, DerivedOutcome, DerivedRule};
use crate::request::{MethodChoice, BODY_MASS_GLOBAL};
use crate::resolution::Resolution;
use crate::response::Quantity;

pub const ID: &str = "qc.jump_type_autodetection.mass_scaled";

/// The entry's own names, and the values it publishes.
pub const ANCHOR_PARAMETER: &str = "anchor_mass_kg";
pub const ANCHOR_DEFAULT_KILOGRAMS: f64 = 87.5;
pub const EXPONENT_PARAMETER: &str = "exponent";
pub const EXPONENT_DEFAULT: f64 = 0.667;
pub const THRESHOLD_PARAMETER: &str = "threshold_n";
pub const THRESHOLD_AT_ANCHOR_DEFAULT_NEWTONS: f64 = 250.0;

/// The one value this rule cannot run without, which the request binds for the whole
/// analysis rather than on this rule's row. A mass the plate could have weighed, so a sweep
/// stating it reaches the rule rather than the refusal.
pub const REQUIRED_GLOBALS: &[(&str, f64)] = &[(BODY_MASS_GLOBAL, 52.0)];

pub const QUANTITIES: &[Quantity] = &[
    Quantity {
        key: super::KEY,
        label: super::CLASSIFICATION_LABEL,
        unit: "boolean",
        computed_by: Some(ID),
        produced_by_construct: None,
    },
    Quantity {
        key: super::UNWEIGHTING_KEY,
        label: super::UNWEIGHTING_LABEL,
        unit: "newtons",
        computed_by: Some(ID),
        produced_by_construct: None,
    },
    Quantity {
        key: super::THRESHOLD_KEY,
        label: super::THRESHOLD_LABEL,
        unit: "newtons",
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
    let anchor_kilograms = resolved.number(ANCHOR_PARAMETER, ANCHOR_DEFAULT_KILOGRAMS);
    let exponent = resolved.number(EXPONENT_PARAMETER, EXPONENT_DEFAULT);
    let threshold_at_anchor_newtons =
        resolved.number(THRESHOLD_PARAMETER, THRESHOLD_AT_ANCHOR_DEFAULT_NEWTONS);

    let Some(body_mass_kilograms) = context.body_mass_kilograms else {
        let bound = resolved.finish();
        return DerivedOutcome::declined(
            bound,
            crate::resolution::RuleRefusal::Refused(Box::new(
                plateforce_core::Refusal::required_parameter_unstated(ID, BODY_MASS_GLOBAL),
            )),
        );
    };
    resolved.record_measured(
        BODY_MASS_GLOBAL,
        body_mass_kilograms,
        crate::resolution::format_number(body_mass_kilograms),
        plateforce_core::provenance::ParameterSource::Stated,
    );

    let unweighting = super::unweighting_newtons(context, ID);
    let bound = resolved.finish();

    match unweighting {
        Ok((system_weight_newtons, minimum_force_newtons)) => super::reported(
            plateforce_core::validity::jump_type_mass_scaled(
                system_weight_newtons,
                minimum_force_newtons,
                body_mass_kilograms,
                anchor_kilograms,
                exponent,
                threshold_at_anchor_newtons,
            ),
            bound,
        ),
        Err(refusal) => DerivedOutcome::declined(bound, refusal),
    }
}
