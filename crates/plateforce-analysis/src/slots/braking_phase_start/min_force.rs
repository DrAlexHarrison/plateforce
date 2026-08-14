//! `phase.braking_start.min_force`: the force nadir following onset.
//!
//! A legitimate boundary in one published phase model and a name collision with the other.
//! It strictly precedes the net-force crossing, so a user importing two products' braking
//! columns is reading two different measurements under one heading.
//!
//! The search is bounded at the propulsive peak. Bounded at takeoff it returns the sample
//! before takeoff, because force is still collapsing toward zero there and never turns back
//! up: one shipped tool computes the boundary both ways and overwrites the first with the
//! second, and only the second reaches its output.

use plateforce_core::phases::braking_start_by_force_minimum;

use crate::binding::{ONSET_CONSTRUCT, TAKEOFF_CONSTRUCT};
use crate::boundaries;
use crate::derived::{DerivedContext, DerivedOutcome, DerivedRule};
use crate::request::MethodChoice;
use crate::resolution::{Resolution, RuleRefusal};
use crate::response::Quantity;

pub const ID: &str = "phase.braking_start.min_force";

pub const QUANTITIES: &[Quantity] = &[Quantity {
    key: super::KEY,
    label: "Start of braking",
    unit: "seconds",
    computed_by: Some(ID),
    produced_by_construct: None,
}];

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

    // The peak bounds the search, so a peak that could not be taken is a search that never
    // ran rather than a recording carrying no nadir. Both faults the peak reports are the
    // recording failing to supply an interval, and they take different repairs.
    let peak = match boundaries::propulsive_peak_index(context, onset, takeoff) {
        Ok(peak) => peak,
        Err(plateforce_core::peak::PeakError::SamplesCarryNoNumber(missing)) => {
            return DerivedOutcome::declined(
                bound,
                RuleRefusal::Refused(Box::new(missing.refusal(ID))),
            )
        }
        Err(plateforce_core::peak::PeakError::EmptySpan { .. }) => {
            return DerivedOutcome::declined(
                bound,
                RuleRefusal::Refused(Box::new(plateforce_core::Refusal::span_selects_no_samples(
                    ID, onset, takeoff,
                ))),
            )
        }
        Err(plateforce_core::peak::PeakError::Smoothing(_)) => {
            unreachable!("a raw maximum does not smooth")
        }
    };

    // The nadir is the least force between the onset and that peak, so a peak standing at the
    // onset leaves the search no samples to be least of.
    let Some(index) = braking_start_by_force_minimum(context.trial.force(), onset, peak) else {
        return DerivedOutcome::declined(
            bound,
            RuleRefusal::Refused(Box::new(plateforce_core::Refusal::span_selects_no_samples(
                ID, onset, peak,
            ))),
        );
    };
    DerivedOutcome {
        values: vec![(super::KEY, Some(context.trial.time_at(index)))],
        placed: vec![(super::PLACED, index)],
        bound,
        refusal: None,
    }
}
