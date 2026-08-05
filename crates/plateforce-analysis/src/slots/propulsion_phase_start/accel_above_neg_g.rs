//! `phase.propulsive.accel_above_neg_g`: the velocity-based-training lineage's partition.
//!
//! The registry's text: the propulsive phase is the portion of the concentric action during
//! which measured acceleration is at or above minus g, the remainder being the braking
//! sub-phase in which the lifter actively decelerates the load.
//!
//! The registry files this against `phase.propulsion_start.zero_velocity` as a homonym rather
//! than a competing method: the jump lineage partitions by velocity sign and this one by
//! acceleration relative to gravity, both are correct for their own purpose, and a build that
//! merged them would merge two literatures. So it is bound here and reports the same key, and
//! a result says which entry answered.
//!
//! What the condition reduces to on a force plate is worth stating exactly, because it is a
//! property of the instrument rather than of the rule. Centre of mass acceleration is
//! measured force less system weight over system mass, so acceleration at or above minus g is
//! measured force at or above zero, which holds at every sample the athlete is in contact
//! for. The rule therefore places this boundary where the concentric action begins, and the
//! interval it names runs to the end of contact. The source measured a barbell through a
//! transducer, where the lifter can pull the bar down harder than gravity and the condition
//! genuinely fails before the lift ends; a plate reading the whole system cannot see that,
//! because a system pulling itself down through the floor would read a negative force.
//!
//! The search is run rather than short-circuited, so the boundary is where the condition was
//! met on this recording rather than where a comment says it will be.

use crate::binding::{ONSET_CONSTRUCT, TAKEOFF_CONSTRUCT};
use crate::boundaries;
use crate::derived::{DerivedContext, DerivedOutcome, DerivedRule};
use crate::request::MethodChoice;
use crate::resolution::Resolution;
use crate::response::Quantity;

pub const ID: &str = "phase.propulsive.accel_above_neg_g";

pub const QUANTITIES: &[Quantity] = &[Quantity {
    key: super::KEY,
    label: "Start of the push",
    unit: "seconds",
    computed_by: Some(ID),
}];

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

    // The concentric action begins where velocity turns positive, which is the interval this
    // rule takes a portion of. Read through the same core function the velocity-sign entry
    // reads, so the two lineages disagree about the condition rather than about the curve.
    let gravity = context.gravity_behind(None);
    let velocity = crate::centre_of_mass::velocity(
        context.trial,
        context.epoch(),
        onset,
        gravity,
        &mut resolved,
    );
    let Some(concentric_start) =
        plateforce_core::phases::velocity_zero_crossing(&velocity, onset, takeoff)
    else {
        return DerivedOutcome::declined(
            resolved.finish(),
            crate::resolution::RuleRefusal::Refused(Box::new(
                plateforce_core::Refusal::no_crossing(
                    ID,
                    "concentric_start_velocity_meters_per_second",
                    0.0,
                    context.trial.time_at(takeoff),
                ),
            )),
        );
    };

    let acceleration = plateforce_core::phases::center_of_mass_acceleration(
        context.trial.force(),
        context.epoch().system_weight_newtons,
        context.epoch().system_mass_kilograms(gravity),
    );
    let floor = -gravity;
    let index = (concentric_start.index..=takeoff.min(acceleration.len().saturating_sub(1)))
        .find(|&sample| acceleration[sample] >= floor);

    boundaries::placed_outcome(context, super::KEY, super::PLACED, index, resolved.finish())
}
