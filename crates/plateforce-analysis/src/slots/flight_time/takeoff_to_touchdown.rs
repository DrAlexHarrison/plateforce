//! `flight_time.takeoff_to_touchdown`: the samples between takeoff and the return above the
//! threshold that placed it, times the sampling interval.

use plateforce_core::Refusal;

use crate::binding::{ONSET_CONSTRUCT, TAKEOFF_CONSTRUCT};
use crate::derived::{DerivedContext, DerivedOutcome, DerivedRule};
use crate::request::MethodChoice;
use crate::resolution::{Resolution, RuleRefusal};
use crate::response::Quantity;

pub const ID: &str = "flight_time.takeoff_to_touchdown";

pub const QUANTITIES: &[Quantity] = &[Quantity {
    key: super::KEY,
    label: "Flight time",
    unit: "seconds",
    computed_by: Some(ID),
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
        choice.claims(),
    );
    let bound = resolved.finish();

    let Some(landmarks) = context.landmarks() else {
        return DerivedOutcome::declined(
            bound,
            context.unavailable(ID, &[ONSET_CONSTRUCT, TAKEOFF_CONSTRUCT]),
        );
    };
    let Some(seconds) = super::seconds(context, &landmarks) else {
        return DerivedOutcome::declined(
            bound,
            RuleRefusal::Refused(Box::new(Refusal::required_parameter_unstated(
                ID,
                super::TOUCHDOWN_FIELD,
            ))),
        );
    };

    DerivedOutcome {
        values: vec![(super::KEY, Some(seconds))],
        placed: Vec::new(),
        bound,
        refusal: None,
    }
}
