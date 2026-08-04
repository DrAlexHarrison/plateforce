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
}];

pub const RULE: DerivedRule = compute;

fn compute(
    context: &DerivedContext,
    choice: &MethodChoice,
    _warnings: &mut Vec<String>,
) -> DerivedOutcome {
    let resolved = Resolution::over(&choice.parameters, &choice.options, choice.claims());
    let bound = resolved.finish();

    // Takeoff and the return to the plate, and nothing else. Reading the three landmarks as a
    // bundle made this rule decline on a recording whose onset rule had found nothing, though
    // the interval it measures begins after takeoff and rests on no onset rule at all: the
    // bundle is only assembled when onset is placed and sits before takeoff. It also put the
    // onset rule and every operator it bound into this number's chain, which is the same
    // untruth read the other way round.
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
