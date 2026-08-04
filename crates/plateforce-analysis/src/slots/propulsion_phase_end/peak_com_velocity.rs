//! `phase.propulsion_end.peak_com_velocity`: propulsion ends at maximum velocity, not takeoff.
//!
//! The mirror of the braking-start boundary, the same two signals read in the other
//! direction: a velocity extremum, or the instant force crosses system weight.
//!
//! The signal is required and has no default. The 243-trial measurement that settled the
//! braking-start default was made at the braking start and says nothing about which signal is
//! steadier here, so a rule that filled this from that measurement would carry it onto a
//! boundary it never touched.

use plateforce_core::phases::{
    propulsion_end_by_force_crossing, propulsion_end_by_velocity_maximum,
};

use crate::binding::{ONSET_CONSTRUCT, TAKEOFF_CONSTRUCT};
use crate::boundaries;
use crate::derived::{DerivedContext, DerivedOutcome, DerivedRule};
use crate::request::MethodChoice;
use crate::resolution::Resolution;
use crate::response::Quantity;
use crate::slots::braking_phase_start;

pub const ID: &str = "phase.propulsion_end.peak_com_velocity";

pub const QUANTITIES: &[Quantity] = &[Quantity {
    key: super::KEY,
    label: "End of propulsion",
    unit: "seconds",
    computed_by: Some(ID),
}];

/// What a caller has to answer before this rule can run, with one value that answers it.
///
/// Not a default. `place` refuses an unstated signal and the entry forbids the default that
/// would quiet it. This is what a surface offering the rule has to ask, and what a check
/// reaching the rule has to supply, so neither has to know the answer by heart.
pub const REQUIRED_OPTIONS: &[(&str, &str)] = &[("search_signal", "velocity_argmax")];

#[derive(Clone, Copy)]
enum SearchSignal {
    VelocityArgmax,
    ForceCrossing,
}

pub const RULE: DerivedRule = place;

fn place(
    context: &DerivedContext,
    choice: &MethodChoice,
    _warnings: &mut Vec<String>,
) -> DerivedOutcome {
    let mut resolved = Resolution::over(&choice.parameters, &choice.options, choice.claims());
    let signal = match resolved.required_enumerated(
        ID,
        "search_signal",
        &[
            ("velocity_argmax", SearchSignal::VelocityArgmax),
            ("force_bw_crossing", SearchSignal::ForceCrossing),
        ],
    ) {
        Ok(signal) => signal,
        Err(refusal) => return DerivedOutcome::declined(resolved.finish(), refusal),
    };

    let (Some(onset), Some(takeoff)) = (context.onset_index(), context.takeoff_index()) else {
        let missing = boundaries::absent(context, &[ONSET_CONSTRUCT, TAKEOFF_CONSTRUCT]);
        return DerivedOutcome::declined(resolved.finish(), context.unavailable(ID, &missing));
    };

    match signal {
        SearchSignal::VelocityArgmax => {
            let velocity = crate::centre_of_mass::velocity(
                context.trial,
                context.epoch(),
                onset,
                context.gravity_meters_per_second_squared,
                &mut resolved,
            );
            let index = propulsion_end_by_velocity_maximum(&velocity, onset, takeoff);
            boundaries::placed_outcome(context, super::KEY, super::PLACED, index, resolved.finish())
        }
        // The falling crossing is searched from the braking start, so under this signal the
        // boundary rests on whichever braking-start rule ran and names it in the chain.
        SearchSignal::ForceCrossing => {
            let Some(braking_start) = braking_phase_start::placed(context) else {
                return DerivedOutcome::declined(
                    resolved.finish(),
                    context.unavailable(ID, &[braking_phase_start::CONSTRUCT]),
                );
            };
            let crossing = propulsion_end_by_force_crossing(
                context.trial.force(),
                context.epoch().system_weight_newtons,
                braking_start,
                takeoff,
            );
            boundaries::crossing_outcome(
                context,
                ID,
                super::KEY,
                super::PLACED,
                crossing,
                resolved.finish(),
            )
        }
    }
}
