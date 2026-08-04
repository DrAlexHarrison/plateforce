//! `phase.model.unweighting_single.mcmahon2018`: one unweighting phase, no force minimum.
//!
//! The unweighting phase is the whole area of the force-time curve before takeoff that lies
//! below system weight. The minimum-force instant is not a boundary in this model, which is
//! the whole of the difference between it and the split model: an implementation that puts the
//! minimum in has built the other one.
//!
//! The cost is measured rather than assumed. Unloading time and unloading rate of force
//! development predict the modified reactive strength index while yielding time and yielding
//! rate do not, so collapsing the two averages a predictive sub-interval with one that is not.

use plateforce_core::phases::{phase_model_unweighting_single, PhaseModelOutcome};

use crate::binding::{ONSET_CONSTRUCT, TAKEOFF_CONSTRUCT};
use crate::boundaries;
use crate::derived::{DerivedContext, DerivedOutcome, DerivedRule};
use crate::request::MethodChoice;
use crate::resolution::Resolution;
use crate::response::Quantity;

pub const ID: &str = "phase.model.unweighting_single.mcmahon2018";

pub const START_KEY: &str = "unweighting_phase_start_seconds";
pub const END_KEY: &str = "unweighting_phase_end_seconds";

/// Two boundaries, which is this model's whole claim about the countermovement.
pub const QUANTITIES: &[Quantity] = &[
    Quantity {
        key: START_KEY,
        label: "Start of unweighting",
        unit: "seconds",
        computed_by: Some(ID),
    },
    Quantity {
        key: END_KEY,
        label: "End of unweighting",
        unit: "seconds",
        computed_by: Some(ID),
    },
];

pub const RULE: DerivedRule = place;

fn place(
    context: &DerivedContext,
    choice: &MethodChoice,
    _warnings: &mut Vec<String>,
) -> DerivedOutcome {
    let resolved = Resolution::over(&choice.parameters, &choice.options, choice.claims());
    let bound = resolved.finish();

    let (Some(onset), Some(takeoff)) = (context.onset_index(), context.takeoff_index()) else {
        let missing = boundaries::absent(context, &[ONSET_CONSTRUCT, TAKEOFF_CONSTRUCT]);
        return DerivedOutcome::declined(bound, context.unavailable(ID, &missing));
    };

    let model = boundaries::propulsive_peak_index(context, onset, takeoff)
        .map(|peak| {
            phase_model_unweighting_single(
                context.trial.force(),
                context.epoch().system_weight_newtons,
                onset,
                peak,
            )
        })
        .unwrap_or(PhaseModelOutcome::NothingToPlace);
    boundaries::model_outcome(context, ID, &[START_KEY, END_KEY], model, bound)
}
