//! `sticking.region.velocity_minimum`: the first local maximum of ascent velocity to the
//! local minimum after it.
//!
//! The registry's text, and its own three warnings, all of which the implementation carries
//! rather than restates. The clause that velocity increases again after the minimum is the
//! one that separates a sticking region from the deceleration at the end of a lift, and it is
//! also the clause that never fires on a deadlift, where the second peak velocity is lower
//! than the first. Both cases return no region, which the entry asks for by name.
//!
//! The quantity is two extrema of a differentiated signal, so it moves with whatever cutoff
//! conditioned the trace. The registry files a squat velocity drop of 0.03 m/s against
//! 0.22 m/s on a bench press, and 0.03 m/s is at or below the noise floor of a
//! plate-integrated velocity: the one lift a force plate could see this on is the one where
//! it is hardest to see. That is recorded against the entry, not written into a comment as a
//! reason not to run.
//!
//! The ascent is the stretch from where velocity turned positive to where the push ended, so
//! this rests on whichever propulsion rules the caller bound.

use plateforce_core::phases::sticking_region_by_velocity_minimum;

use crate::binding::{ONSET_CONSTRUCT, TAKEOFF_CONSTRUCT};
use crate::boundaries;
use crate::derived::{DerivedContext, DerivedOutcome, DerivedRule};
use crate::request::MethodChoice;
use crate::resolution::Resolution;
use crate::response::Quantity;
use crate::slots::{propulsion_phase_end, propulsion_phase_start};

pub const ID: &str = "sticking.region.velocity_minimum";

pub const QUANTITIES: &[Quantity] = &[
    Quantity {
        key: super::START_KEY,
        label: "Where the bar slows down, start",
        unit: "seconds",
        computed_by: Some(ID),
    },
    Quantity {
        key: super::END_KEY,
        label: "Where the bar slows down, end",
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
    let mut resolved = Resolution::over(&choice.parameters, &choice.options, choice.claims());

    let (Some(onset), Some(takeoff)) = (context.onset_index(), context.takeoff_index()) else {
        let missing = boundaries::absent(context, &[ONSET_CONSTRUCT, TAKEOFF_CONSTRUCT]);
        return DerivedOutcome::declined(resolved.finish(), context.unavailable(ID, &missing));
    };

    // The ascent runs between the propulsion boundaries where a caller bound rules for them,
    // and between onset and takeoff where they did not. Reading the bound boundaries is what
    // puts them into this number's chain, so a region measured over one interval cannot be
    // compared with one measured over another without the record saying so.
    let ascent_start = propulsion_phase_start::placed(context).unwrap_or(onset);
    let ascent_end = propulsion_phase_end::placed(context).unwrap_or(takeoff);

    let velocity = crate::centre_of_mass::velocity(
        context.trial,
        context.epoch(),
        onset,
        context.gravity_behind(None),
        &mut resolved,
    );
    let bound = resolved.finish();

    let region =
        sticking_region_by_velocity_minimum(velocity.meters_per_second(), ascent_start, ascent_end);

    // The not-detected state the entry asks for, carrying the interval it read rather than
    // two blank keys. A lifter with no sticking point and a rule that never ran reach a
    // reader identically as a pair of empty values, and they are different facts: this one is
    // the search running over a named stretch and finding no pair of extrema in it.
    //
    // A countermovement jump is that case by construction. Velocity rises monotonically from
    // the zero crossing to its peak, so there is no local maximum inside the push for a
    // minimum to follow, and both committed traces answer this way. The recording that would
    // settle the rule is a maximal or near-maximal bench press or squat, where the published
    // velocity drop is 0.22 and 0.03 m/s respectively.
    let Some(found) = region else {
        return DerivedOutcome::declined(
            bound,
            crate::resolution::RuleRefusal::Refused(Box::new(
                plateforce_core::Refusal::nothing_qualified(
                    ID,
                    (ascent_end + 1).saturating_sub(ascent_start),
                    std::collections::BTreeMap::from([
                        (
                            "ascent_start_seconds".to_string(),
                            context.trial.time_at(ascent_start),
                        ),
                        (
                            "ascent_end_seconds".to_string(),
                            context.trial.time_at(ascent_end),
                        ),
                    ]),
                ),
            )),
        );
    };

    DerivedOutcome {
        values: vec![
            (
                super::START_KEY,
                Some(context.trial.time_at(found.start_index)),
            ),
            (super::END_KEY, Some(context.trial.time_at(found.end_index))),
        ],
        placed: vec![
            (super::START_PLACED, found.start_index),
            (super::END_PLACED, found.end_index),
        ],
        bound,
        refusal: None,
    }
}
