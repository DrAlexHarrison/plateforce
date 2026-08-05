//! `phase.braking_start.zero_net_force`: net force crosses zero upward after the minimum.
//!
//! The registry's text: peak negative centre of mass velocity, force returning to system
//! weight, and zero acceleration are the same instant, since velocity is stationary exactly
//! where net force is zero. Definitionally equivalent and numerically consequential, so which
//! signal the search reads is a bound choice rather than an implementation detail.
//!
//! The velocity minimum is uniformly soft, about 33 ms wide at the 1 percent level, and
//! produces no cross-trial error variance. The multi-valued force crossing does: disagreement
//! magnitude correlates with raw-force re-crossings at r = +0.640 and with velocity flat-band
//! width at r = -0.062, measured on 243 trials.

use plateforce_core::phases::{braking_start_by_force_return, braking_start_by_velocity_minimum};

use crate::binding::{ONSET_CONSTRUCT, TAKEOFF_CONSTRUCT};
use crate::boundaries;
use crate::derived::{DerivedContext, DerivedOutcome, DerivedRule};
use crate::request::MethodChoice;
use crate::resolution::Resolution;
use crate::response::Quantity;

pub const ID: &str = "phase.braking_start.zero_net_force";

pub const QUANTITIES: &[Quantity] = &[Quantity {
    key: super::KEY,
    label: "Start of braking",
    unit: "seconds",
    computed_by: Some(ID),
}];

/// Which signal the search reads, as the registry's parameter spells its values.
#[derive(Clone, Copy)]
enum SearchSignal {
    VelocityArgmin,
    ForceCrossing,
}

pub const RULE: DerivedRule = place;

fn place(
    context: &DerivedContext,
    choice: &MethodChoice,
    _warnings: &mut Vec<String>,
) -> DerivedOutcome {
    let mut resolved = Resolution::over(&choice.parameters, &choice.options, choice.claims());
    let signal = match resolved.enumerated(
        "search_signal",
        "velocity_argmin",
        &[
            ("velocity_argmin", SearchSignal::VelocityArgmin),
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
        SearchSignal::VelocityArgmin => {
            let velocity = crate::centre_of_mass::velocity(
                context.trial,
                context.epoch(),
                onset,
                context.gravity_behind(None),
                &mut resolved,
            );
            let index = braking_start_by_velocity_minimum(&velocity, onset, takeoff);
            boundaries::placed_outcome(context, super::KEY, super::PLACED, index, resolved.finish())
        }
        // Bounded at the propulsive peak. Bounded at takeoff the search anchors on the force
        // collapse rather than on the unweighting minimum and returns takeoff itself.
        SearchSignal::ForceCrossing => {
            let crossing =
                boundaries::propulsive_peak_index(context, onset, takeoff).and_then(|peak| {
                    braking_start_by_force_return(
                        context.trial.force(),
                        onset,
                        context.epoch().system_weight_newtons,
                        peak,
                    )
                });
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
