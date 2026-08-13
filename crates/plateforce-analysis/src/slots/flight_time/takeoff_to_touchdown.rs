//! `flight_time.takeoff_to_touchdown`: the samples between takeoff and the return above the
//! threshold that placed it, times the sampling interval.

use plateforce_core::Refusal;

use crate::binding::TAKEOFF_CONSTRUCT;
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

    // Takeoff and the return to the plate, and nothing else. This interval begins after
    // takeoff and rests on no onset rule, so it reads the two landmarks it uses rather than
    // the three-landmark bundle.
    let Some(takeoff_index) = context.takeoff_index() else {
        return DerivedOutcome::declined(bound, context.unavailable(ID, &[TAKEOFF_CONSTRUCT]));
    };
    let Some(seconds) = super::seconds(context, takeoff_index) else {
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
