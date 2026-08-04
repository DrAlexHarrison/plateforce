//! `phase.propulsion_subdivision.by_force_crossing`: split where force descends through
//! system weight.
//!
//! Event-anchored rather than arbitrary, and it coincides with peak centre of mass velocity
//! and with zero centre of mass displacement, giving acceleration and deceleration sub-phases.
//! The time split beside it is equally published and lands somewhere else on the same
//! interval.

use plateforce_core::phases::propulsion_subdivision_by_force_crossing;

use crate::boundaries;
use crate::derived::{DerivedContext, DerivedOutcome, DerivedRule};
use crate::request::MethodChoice;
use crate::resolution::Resolution;
use crate::response::Quantity;
use crate::slots::{propulsion_phase_end, propulsion_phase_start};

pub const ID: &str = "phase.propulsion_subdivision.by_force_crossing";

pub const QUANTITIES: &[Quantity] = &[Quantity {
    key: super::propulsion_subdivision_by_time::KEY,
    label: "Propulsion split",
    unit: "seconds",
    computed_by: Some(ID),
}];

pub const RULE: DerivedRule = place;

fn place(
    context: &DerivedContext,
    choice: &MethodChoice,
    _warnings: &mut Vec<String>,
) -> DerivedOutcome {
    let resolved = Resolution::over(&choice.parameters, &choice.options, choice.claims());
    let bound = resolved.finish();

    let (Some(start), Some(end)) = (
        propulsion_phase_start::placed(context),
        propulsion_phase_end::placed(context),
    ) else {
        return DerivedOutcome::declined(
            bound,
            context.unavailable(
                ID,
                &[
                    propulsion_phase_start::CONSTRUCT,
                    propulsion_phase_end::CONSTRUCT,
                ],
            ),
        );
    };

    let index = propulsion_subdivision_by_force_crossing(
        context.trial.force(),
        context.epoch().system_weight_newtons,
        start,
        end,
    );
    boundaries::crossing_or_refusal(
        context,
        ID,
        super::propulsion_subdivision_by_time::KEY,
        super::propulsion_subdivision_by_time::PLACED,
        index,
        bound,
    )
}
