//! `vocab.downward_upward`: two intervals, named downward and upward.
//!
//! The registry's text: refer to the countermovement as the downward phase and to propulsion
//! as the upward phase, avoiding eccentric and concentric and, by extension, any ratio named
//! after them. Two sources propose it independently, and one of them additionally avoids
//! braking and propulsive.
//!
//! It is filed under `phase_model` because that construct holds two questions, which
//! landmarks are promoted and what the intervals are called, and this entry answers the
//! second. It answers the first as well, and minimally: one boundary, the velocity sign
//! change, splitting the movement in two. Neither the force minimum nor the departure below
//! system weight is promoted, which is what makes it a different partition from both
//! countermovement models rather than a relabelling of one of them.
//!
//! So the keys are the vocabulary. A reader who chose this model reads
//! `downward_phase_start_seconds` where another reads `unweighting_phase_start_seconds`, and
//! the two are different intervals as well as different words: unweighting begins where force
//! departs below system weight, and the downward phase begins at onset.
//!
//! The physiological argument behind the naming is sound and the registry states it: fascicles
//! usually do not actively lengthen during the countermovement, so eccentric phase is a claim
//! about muscle behaviour the force trace cannot support. Adoption is low, which is the whole
//! reason the vocabulary is a choice a reader makes rather than one this build makes for them.

use crate::binding::{ONSET_CONSTRUCT, TAKEOFF_CONSTRUCT};
use crate::boundaries;
use crate::derived::{DerivedContext, DerivedOutcome, DerivedRule};
use crate::request::MethodChoice;
use crate::resolution::Resolution;
use crate::response::Quantity;

pub const ID: &str = "vocab.downward_upward";

pub const DOWNWARD_START_KEY: &str = "downward_phase_start_seconds";
pub const DOWNWARD_END_KEY: &str = "downward_phase_end_seconds";
pub const UPWARD_END_KEY: &str = "upward_phase_end_seconds";

/// Three instants bounding two intervals, under the names this entry exists to establish.
/// The upward phase starts where the downward phase ends, so that instant is reported once.
pub const QUANTITIES: &[Quantity] = &[
    Quantity {
        key: DOWNWARD_START_KEY,
        label: "Start of the downward phase",
        unit: "seconds",
        computed_by: Some(ID),
        produced_by_construct: None,
    },
    Quantity {
        key: DOWNWARD_END_KEY,
        label: "End of the downward phase, start of the upward phase",
        unit: "seconds",
        computed_by: Some(ID),
        produced_by_construct: None,
    },
    Quantity {
        key: UPWARD_END_KEY,
        label: "End of the upward phase",
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
    let mut resolved = Resolution::over(
        &choice.parameters,
        &choice.options,
        choice.declared.of_entry(ID),
        choice.claims(),
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
    let turn = plateforce_core::phases::velocity_zero_crossing(&velocity, onset, takeoff);
    let bound = resolved.finish();

    // The one boundary this model promotes. A search that returned an index without the
    // velocity having crossed would put a turn where the athlete never turned, so the model
    // places nothing and says which search found nothing.
    let Some(turn) = turn.filter(|crossing| crossing.is_true_crossing) else {
        return DerivedOutcome::declined(
            bound,
            crate::resolution::RuleRefusal::Refused(Box::new(
                plateforce_core::Refusal::no_crossing(
                    ID,
                    "downward_to_upward_velocity_meters_per_second",
                    0.0,
                    context.trial.time_at(takeoff),
                ),
            )),
        );
    };

    DerivedOutcome {
        values: vec![
            (DOWNWARD_START_KEY, Some(context.trial.time_at(onset))),
            (DOWNWARD_END_KEY, Some(context.trial.time_at(turn.index))),
            (UPWARD_END_KEY, Some(context.trial.time_at(takeoff))),
        ],
        placed: Vec::new(),
        bound,
        refusal: None,
    }
}
