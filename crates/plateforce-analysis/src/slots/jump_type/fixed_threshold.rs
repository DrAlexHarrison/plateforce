//! `qc.jump_type_autodetection.sams`: a countermovement jump is one that unloaded the plate
//! by more than a fixed number of newtons.
//!
//! The constant is retained unmodified at the value its author disclosed as arbitrary,
//! because reproducing that tool's output needs the exact number. His own disclosure is that
//! it worked well for athletes from 65 to 110 kg and may not for smaller ones, and the
//! registry carries that beside the answer rather than behind it.

use crate::derived::{DerivedContext, DerivedOutcome, DerivedRule};
use crate::request::MethodChoice;
use crate::resolution::Resolution;
use crate::response::Quantity;

pub const ID: &str = "qc.jump_type_autodetection.sams";

/// The entry's own name for the threshold, and the value it publishes.
pub const THRESHOLD_PARAMETER: &str = "threshold_n";
pub const THRESHOLD_DEFAULT_NEWTONS: f64 = 250.0;

pub const QUANTITIES: &[Quantity] = &[
    Quantity {
        key: super::KEY,
        label: super::CLASSIFICATION_LABEL,
        unit: "boolean",
        computed_by: Some(ID),
    },
    Quantity {
        key: super::UNWEIGHTING_KEY,
        label: super::UNWEIGHTING_LABEL,
        unit: "newtons",
        computed_by: Some(ID),
    },
    Quantity {
        key: super::THRESHOLD_KEY,
        label: super::THRESHOLD_LABEL,
        unit: "newtons",
        computed_by: Some(ID),
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
    let threshold_newtons = resolved.number(THRESHOLD_PARAMETER, THRESHOLD_DEFAULT_NEWTONS);
    let unweighting = super::unweighting_newtons(context, ID);
    let bound = resolved.finish();

    match unweighting {
        Ok((system_weight_newtons, minimum_force_newtons)) => super::reported(
            plateforce_core::validity::jump_type_fixed_threshold(
                system_weight_newtons,
                minimum_force_newtons,
                threshold_newtons,
            ),
            bound,
        ),
        Err(refusal) => DerivedOutcome::declined(bound, refusal),
    }
}
