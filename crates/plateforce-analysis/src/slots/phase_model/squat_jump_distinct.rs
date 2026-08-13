//! `phase.model.squat_jump_distinct`: the squat jump has no countermovement landmarks.
//!
//! The registry's text: the model reduces to static hold, propulsion from onset to takeoff,
//! flight and landing. There is no unweighting, no force minimum, no peak negative velocity
//! and no zero-velocity transition, because the athlete descends into a semi-squat and holds
//! it before initiating a purely upward movement.
//!
//! So the keys this publishes are the whole of its claim, and what it does not publish is the
//! rest of it. A caller who chose this model and looks for a force minimum finds no key for
//! one, which is the model saying the landmark does not exist rather than the search failing.
//! That is why `phase_model` is declared `their_own_questions`: the two countermovement
//! models publish two and five boundaries and this one publishes two of its own.
//!
//! Whether the recording in front of it is a squat jump is `jump_type`, a different
//! construct, and detecting a contaminating countermovement is a method variant of its own by
//! the registry's own account. This model does not test for one. The hazard it records is
//! real and measured elsewhere: a small countermovement can enhance squat-jump height by up
//! to 6 cm in elite athletes.

use crate::binding::{ONSET_CONSTRUCT, TAKEOFF_CONSTRUCT};
use crate::boundaries;
use crate::derived::{DerivedContext, DerivedOutcome, DerivedRule};
use crate::request::MethodChoice;
use crate::resolution::Resolution;
use crate::response::Quantity;

pub const ID: &str = "phase.model.squat_jump_distinct";

pub const STATIC_HOLD_END_KEY: &str = "static_hold_end_seconds";
pub const PROPULSION_END_KEY: &str = "squat_jump_propulsion_end_seconds";

/// Two boundaries and no countermovement landmark between them, which is the model.
pub const QUANTITIES: &[Quantity] = &[
    Quantity {
        key: STATIC_HOLD_END_KEY,
        label: "End of the static hold",
        unit: "seconds",
        computed_by: Some(ID),
        produced_by_construct: None,
    },
    Quantity {
        key: PROPULSION_END_KEY,
        label: "End of propulsion",
        unit: "seconds",
        computed_by: Some(ID),
        produced_by_construct: None,
    },
];

pub const RULE: DerivedRule = place;

fn place(
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

    let (Some(onset), Some(takeoff)) = (context.onset_index(), context.takeoff_index()) else {
        let missing = boundaries::absent(context, &[ONSET_CONSTRUCT, TAKEOFF_CONSTRUCT]);
        return DerivedOutcome::declined(bound, context.unavailable(ID, &missing));
    };

    // The hold ends where the push begins, which under this model is the onset itself: there
    // is no descent to separate the two. Reading the bound onset rule rather than searching
    // is the model's own position, and it means the boundary moves with the onset rule the
    // caller chose and the record says so.
    DerivedOutcome {
        values: vec![
            (STATIC_HOLD_END_KEY, Some(context.trial.time_at(onset))),
            (PROPULSION_END_KEY, Some(context.trial.time_at(takeoff))),
        ],
        placed: Vec::new(),
        bound,
        refusal: None,
    }
}
