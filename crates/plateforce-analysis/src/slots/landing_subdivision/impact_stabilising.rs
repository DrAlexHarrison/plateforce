//! `phase.landing_subdivision.impact_stabilising`: the landing splits at peak force.
//!
//! The registry's text: impact runs from touchdown to peak force, stabilising from peak force
//! to peak negative displacement. The split is the peak, and the two sub-phases are the
//! stretches either side of it.
//!
//! Peak force here is the largest force during the landing and not the largest in the
//! recording, which on an untrimmed trace are the same sample and on a trimmed one are not.
//! The search is bounded at touchdown for that reason: the propulsive peak of the jump before
//! it is larger on some traces and smaller on others, and a split placed there would divide
//! the flight rather than the landing.
//!
//! The far bound is where the landing ended, so this rule rests on whichever landing-end rule
//! ran. A split placed at either end of the interval divides it into all of it and none of
//! it, which `subdivision_outcome` refuses rather than reports.

use crate::boundaries;
use crate::derived::{DerivedContext, DerivedOutcome, DerivedRule};
use crate::request::MethodChoice;
use crate::resolution::Resolution;
use crate::response::Quantity;
use crate::slots::{landing, landing_phase_end};

pub const ID: &str = "phase.landing_subdivision.impact_stabilising";

pub const QUANTITIES: &[Quantity] = &[Quantity {
    key: super::KEY,
    label: "Where the landing is split in two",
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

    let Some(touchdown) = landing::placed(context).or_else(|| context.touchdown_index()) else {
        return DerivedOutcome::declined(bound, context.unavailable(ID, &[landing::CONSTRUCT]));
    };
    let Some(end) = landing_phase_end::placed(context) else {
        return DerivedOutcome::declined(
            bound,
            context.unavailable(ID, &[landing_phase_end::CONSTRUCT]),
        );
    };

    let peak =
        plateforce_core::peak::index_of_maximum_over(context.trial.force(), touchdown, end).ok();

    boundaries::subdivision_outcome(
        context,
        ID,
        super::KEY,
        super::PLACED,
        (touchdown, end),
        peak,
        bound,
    )
}
