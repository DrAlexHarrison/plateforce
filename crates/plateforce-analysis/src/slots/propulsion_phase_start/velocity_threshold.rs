//! `phase.propulsion_start.velocity_threshold`: velocity first exceeds a small positive value.
//!
//! A noise guard rather than a different account of where the phase begins: a bare zero
//! crossing can fire on numerical jitter. It is not a pure implementation detail either,
//! because it creates a nameable amortisation interval between zero velocity and the
//! threshold that the zero-crossing rule does not have, and the source states outright that
//! no study has explored it.

use plateforce_core::phases::velocity_threshold_crossing;

use crate::binding::{ONSET_CONSTRUCT, TAKEOFF_CONSTRUCT};
use crate::boundaries;
use crate::derived::{DerivedContext, DerivedOutcome, DerivedRule};
use crate::request::MethodChoice;
use crate::resolution::Resolution;
use crate::response::Quantity;

pub const ID: &str = "phase.propulsion_start.velocity_threshold";

/// The only value the entry publishes, used on full-squad datasets by the authors.
const PUBLISHED_THRESHOLD_METERS_PER_SECOND: f64 = 0.01;

pub const QUANTITIES: &[Quantity] = &[Quantity {
    key: super::KEY,
    label: "Start of propulsion",
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
    let threshold_meters_per_second =
        resolved.number("threshold_mps", PUBLISHED_THRESHOLD_METERS_PER_SECOND);

    let (Some(onset), Some(takeoff)) = (context.onset_index(), context.takeoff_index()) else {
        let missing = boundaries::absent(context, &[ONSET_CONSTRUCT, TAKEOFF_CONSTRUCT]);
        return DerivedOutcome::declined(resolved.finish(), context.unavailable(ID, &missing));
    };
    let velocity = crate::centre_of_mass::velocity(
        context.trial,
        context.epoch(),
        onset,
        context.gravity_behind(Some(super::KEY)),
        &mut resolved,
    );
    let crossing =
        velocity_threshold_crossing(&velocity, onset, takeoff, threshold_meters_per_second);
    let bound = resolved.finish();

    boundaries::crossing_outcome(context, ID, super::KEY, super::PLACED, crossing, bound)
}
