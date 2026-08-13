//! `time_to_takeoff.onset_to_takeoff`: the samples between the two placed landmarks, times the
//! sampling interval.

use plateforce_core::time_to_takeoff_seconds;

use crate::binding::{ONSET_CONSTRUCT, TAKEOFF_CONSTRUCT};
use crate::derived::{DerivedContext, DerivedOutcome, DerivedRule};
use crate::request::MethodChoice;
use crate::resolution::Resolution;
use crate::response::Quantity;

pub const ID: &str = "time_to_takeoff.onset_to_takeoff";

pub const QUANTITIES: &[Quantity] = &[Quantity {
    key: super::KEY,
    label: "Time to takeoff",
    unit: "seconds",
    computed_by: Some(ID),
    produced_by_construct: None,
}];

pub const RULE: DerivedRule = compute;

fn compute(
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

    let Some(landmarks) = context.landmarks() else {
        return DerivedOutcome::declined(
            bound,
            context.unavailable(ID, &[ONSET_CONSTRUCT, TAKEOFF_CONSTRUCT]),
        );
    };

    DerivedOutcome {
        values: vec![(
            super::KEY,
            Some(time_to_takeoff_seconds(
                &landmarks,
                context.trial.sample_interval_seconds(),
            )),
        )],
        placed: Vec::new(),
        bound,
        refusal: None,
    }
}
