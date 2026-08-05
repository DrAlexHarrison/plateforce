use plateforce_core::QuadratureRule;

use crate::derived::{DerivedContext, DerivedOutcome, DerivedRule};
use crate::request::MethodChoice;
use crate::response::Quantity;
use crate::slots::net_impulse;

pub const ID: &str = "integration.rule.rectangle";

pub const QUANTITIES: &[Quantity] = &[
    Quantity {
        key: net_impulse::KEY,
        label: "Net impulse",
        unit: "newton_seconds",
        computed_by: Some(ID),
    },
    Quantity {
        key: net_impulse::VELOCITY_KEY,
        label: "Takeoff velocity",
        unit: "meters_per_second",
        computed_by: Some(ID),
    },
];

pub const RULE: DerivedRule = compute;

fn compute(
    context: &DerivedContext,
    choice: &MethodChoice,
    _warnings: &mut Vec<String>,
) -> DerivedOutcome {
    super::compute(context, choice, ID, QuadratureRule::Rectangle)
}
