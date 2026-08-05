//! `phase.model.unloading_yielding_split.harry2020`: five boundaries, split at the minimum.
//!
//! Unloading runs from this model's own definition of where movement began, force decreasing
//! by more than a stated percentage of system weight, to the local force minimum after it.
//! Eccentric yielding runs from that minimum to peak negative velocity, eccentric braking from
//! there to positive velocity, and the concentric phase to takeoff.
//!
//! The unloading start is read from the model's own drop rather than from the bound onset rule,
//! so a caller changing the onset rule does not move a boundary this model did not ask that
//! rule for. The unloading phase is strictly shorter than the single unweighting phase, so an
//! eccentric rate of force development compared across the two models is comparing intervals.

use plateforce_core::phases::{phase_model_unloading_yielding_split, PhaseModelOutcome};

use crate::binding::{ONSET_CONSTRUCT, TAKEOFF_CONSTRUCT};
use crate::boundaries;
use crate::derived::{DerivedContext, DerivedOutcome, DerivedRule};
use crate::request::MethodChoice;
use crate::resolution::Resolution;
use crate::response::Quantity;

pub const ID: &str = "phase.model.unloading_yielding_split.harry2020";

/// The only value the entry publishes.
const PUBLISHED_DROP_PERCENT_OF_SYSTEM_WEIGHT: f64 = 2.5;

pub const UNLOADING_START_KEY: &str = "unloading_phase_start_seconds";
pub const FORCE_MINIMUM_KEY: &str = "force_minimum_seconds";
pub const PEAK_NEGATIVE_VELOCITY_KEY: &str = "peak_negative_velocity_seconds";
pub const POSITIVE_VELOCITY_KEY: &str = "positive_velocity_seconds";
pub const CONCENTRIC_END_KEY: &str = "concentric_phase_end_seconds";

/// Five keys against the single-phase model's two. The count is the disagreement.
pub const QUANTITIES: &[Quantity] = &[
    Quantity {
        key: UNLOADING_START_KEY,
        label: "Start of unloading",
        unit: "seconds",
        computed_by: Some(ID),
    },
    Quantity {
        key: FORCE_MINIMUM_KEY,
        label: "Force minimum",
        unit: "seconds",
        computed_by: Some(ID),
    },
    Quantity {
        key: PEAK_NEGATIVE_VELOCITY_KEY,
        label: "Peak negative velocity",
        unit: "seconds",
        computed_by: Some(ID),
    },
    Quantity {
        key: POSITIVE_VELOCITY_KEY,
        label: "Velocity turns positive",
        unit: "seconds",
        computed_by: Some(ID),
    },
    Quantity {
        key: CONCENTRIC_END_KEY,
        label: "End of the concentric phase",
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
    let mut resolved = Resolution::over(
        &choice.parameters,
        &choice.options,
        choice.declared.of_entry(ID),
        choice.claims(),
    );
    let drop_percent_of_system_weight = resolved.number(
        "unloading_drop_percent_of_system_weight",
        PUBLISHED_DROP_PERCENT_OF_SYSTEM_WEIGHT,
    );

    let (Some(onset), Some(takeoff)) = (context.onset_index(), context.takeoff_index()) else {
        let missing = boundaries::absent(context, &[ONSET_CONSTRUCT, TAKEOFF_CONSTRUCT]);
        return DerivedOutcome::declined(resolved.finish(), context.unavailable(ID, &missing));
    };
    let velocity = crate::centre_of_mass::velocity(
        context.trial,
        context.epoch(),
        onset,
        context.gravity_behind(None),
        &mut resolved,
    );
    // The search runs from the first sample rather than from the bound onset, because the
    // model defines its own start and reading the bound rule's would move every boundary
    // below it with a choice this model does not make.
    let model = boundaries::propulsive_peak_index(context, onset, takeoff)
        .map(|peak| {
            phase_model_unloading_yielding_split(
                context.trial.force(),
                &velocity,
                context.epoch().system_weight_newtons,
                drop_percent_of_system_weight,
                0,
                peak,
                takeoff,
            )
        })
        .unwrap_or(PhaseModelOutcome::NothingToPlace);
    boundaries::model_outcome(context, ID, QUANTITIES, model, resolved.finish())
}
