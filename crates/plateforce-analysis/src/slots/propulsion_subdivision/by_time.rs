//! `phase.propulsion_subdivision.by_time`: split propulsion at a stated share of its duration.
//!
//! Arbitrary but reproducible, against the event-anchored split beside it. Both are published
//! partitions of the same interval and they are not equivalent, so any metric named after a
//! sub-phase depends on which one ran.
//!
//! The interval it splits is whatever the bound propulsion rules placed, so this boundary
//! moves when either of them does and its chain names both.

use plateforce_core::phases::propulsion_subdivision_by_time;

use crate::boundaries;
use crate::derived::{DerivedContext, DerivedOutcome, DerivedRule};
use crate::request::MethodChoice;
use crate::resolution::Resolution;
use crate::response::Quantity;
use crate::slots::{propulsion_phase_end, propulsion_phase_start};

pub const ID: &str = "phase.propulsion_subdivision.by_time";

/// The only value the entry publishes.
const PUBLISHED_SPLIT_PERCENT_OF_DURATION: f64 = 50.0;

pub const KEY: &str = "propulsion_subdivision_seconds";

pub const QUANTITIES: &[Quantity] = &[Quantity {
    key: KEY,
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
    let mut resolved = Resolution::over(&choice.parameters, &choice.options, choice.claims());
    let split_percent_of_duration = resolved.number(
        "split_percent_of_duration",
        PUBLISHED_SPLIT_PERCENT_OF_DURATION,
    );
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

    let index = propulsion_subdivision_by_time(start, end, split_percent_of_duration);
    // A stated share is a caller's number, and 0 or 100 of it names an end of the interval
    // rather than a split of it. Held to the same definition of inside as the crossing rule.
    boundaries::subdivision_outcome(context, ID, KEY, PLACED, (start, end), index, bound)
}

/// The instant the sub-phase metrics are bounded by.
pub const PLACED: &str = "propulsion_subdivision";
