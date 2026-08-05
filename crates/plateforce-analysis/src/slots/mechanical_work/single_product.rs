//! `work.single_force_displacement_product`: one force value multiplied by one displacement
//! value, the vendor construction.
//!
//! The manual's own phrasing is internally muddled and the registry says so. What it describes
//! is a peak force per cycle multiplied by the displacement of that cycle, which equals the
//! integral only where force is constant through the displacement. During a jump it is not, so
//! the product is biased and the sign of the bias depends on the shape of the force
//! displacement curve rather than being a constant a reader could correct for.
//!
//! It reports under the same key as the two integral routes, because it claims to be the same
//! quantity. What separates it from them is the number, and the record says which rule produced
//! it.

use crate::binding::ONSET_CONSTRUCT;
use crate::centre_of_mass;
use crate::derived::{DerivedContext, DerivedOutcome, DerivedRule};
use crate::request::MethodChoice;
use crate::resolution::{Resolution, RuleRefusal};
use crate::response::Quantity;
use crate::slots::mechanical_power;

pub const ID: &str = "work.single_force_displacement_product";

/// The one name this rule cannot run without. It forms no power series, so neither the force
/// term nor the sign convention reaches it.
pub const REQUIRED_OPTIONS: &[(&str, &str)] = &[(
    mechanical_power::PHASE_PARAMETER,
    mechanical_power::PHASE_PROPULSION,
)];

pub const QUANTITIES: &[Quantity] = &[Quantity {
    key: super::KEY,
    label: "Work",
    unit: "joules",
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

    let Some(onset) = context.onset_index() else {
        return DerivedOutcome::declined(
            resolved.finish(),
            context.unavailable(ID, &[ONSET_CONSTRUCT]),
        );
    };
    let phase = mechanical_power::phase_interval(context, &mut resolved, ID);
    // The displacement is the velocity integrated a second time, which is what the manual's
    // "integrate velocity to obtain displacement" names, so the four integration choices are
    // recorded on this rule exactly as they are on the routes that integrate power.
    let displacement = centre_of_mass::displacement(
        context.trial,
        context.epoch(),
        onset,
        context.gravity_behind(Some(super::KEY)),
        &mut resolved,
    );
    let bound = resolved.finish();

    let phase = match phase {
        Ok(phase) => phase,
        Err(refusal) => return DerivedOutcome::declined(bound, refusal),
    };

    mechanical_power::record_entries_behind(context, super::KEY, onset);
    match plateforce_core::power::work_from_single_force_displacement_product_joules(
        context.trial.force(),
        displacement.meters(),
        &phase,
    ) {
        Ok(joules) => DerivedOutcome {
            values: vec![(super::KEY, Some(joules))],
            placed: Vec::new(),
            bound,
            refusal: None,
        },
        Err(_) => DerivedOutcome::declined(
            bound,
            RuleRefusal::Refused(Box::new(plateforce_core::Refusal::span_selects_no_samples(
                ID,
                phase.first_index,
                phase.last_index,
            ))),
        ),
    }
}
