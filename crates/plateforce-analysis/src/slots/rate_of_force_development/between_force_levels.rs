//! `rfd.between_force_levels`: the elapsed time from one force level to another, and the
//! slope that interval implies.
//!
//! Anchored on force rather than on time, so it compares the same force range in a strong and
//! a weak athlete where a fixed time window compares different ones, and it is independent of
//! where onset was placed. Both numbers are one rule: the reciprocal of the interval, scaled
//! by the force between the levels, is the slope.
//!
//! All three parameters are required and the registry publishes no value for any of them. No
//! published pair of levels was located, so a pair chosen here would be a silent default
//! sitting where the method's whole content is, and an unstated level is refused by name.
//!
//! `fraction_of_mvc` reads its levels from a separate maximal trial. This analysis is handed
//! one recording, so the basis is refused by name with the basis this rule does read beside
//! it, rather than being served from the peak of the trial in hand, which is a different
//! quantity.

use crate::derived::{DerivedContext, DerivedOutcome, DerivedRule};
use crate::request::MethodChoice;
use crate::resolution::{Resolution, RuleRefusal};
use crate::response::Quantity;
use crate::slots::analysis_window;

pub const ID: &str = "rfd.between_force_levels";

/// The entry's own names for its three required values, and the one basis this rule reads.
pub const BASIS_PARAMETER: &str = "reference_basis";
pub const LOWER_PARAMETER: &str = "lower_level";
pub const UPPER_PARAMETER: &str = "upper_level";
pub const ABSOLUTE_BASIS: &str = "absolute";

pub const QUANTITIES: &[Quantity] = &[Quantity {
    key: super::KEY,
    label: "Rate of force development",
    unit: "newtons_per_second",
    computed_by: Some(ID),
}];

pub const RULE: DerivedRule = compute;

fn compute(
    context: &DerivedContext,
    choice: &MethodChoice,
    _warnings: &mut Vec<String>,
) -> DerivedOutcome {
    let mut resolved = Resolution::over(&choice.parameters, &choice.options, choice.claims());
    // All three are consulted before any of them is judged, so a request stating two of them
    // and omitting the third does not report the two it stated as names this rule never read.
    let basis = resolved.required_enumerated(ID, BASIS_PARAMETER, &[(ABSOLUTE_BASIS, ())]);
    let lower = resolved.required_number(ID, LOWER_PARAMETER);
    let upper = resolved.required_number(ID, UPPER_PARAMETER);
    let bound = resolved.finish();

    let (lower_newtons, upper_newtons) = match (basis, lower, upper) {
        (Ok(()), Ok(lower), Ok(upper)) => (lower, upper),
        (Err(refusal), _, _) | (_, Err(refusal), _) | (_, _, Err(refusal)) => {
            return DerivedOutcome::declined(bound, refusal)
        }
    };

    if upper_newtons <= lower_newtons {
        return DerivedOutcome::declined(
            bound,
            RuleRefusal::Refused(Box::new(plateforce_core::Refusal::value_not_accepted(
                ID,
                UPPER_PARAMETER,
                upper_newtons,
                vec![format!(
                    "a level above {LOWER_PARAMETER}, which is {lower_newtons} N"
                )],
            ))),
        );
    }

    let Some((start, end)) = analysis_window::span(context) else {
        return DerivedOutcome::declined(
            bound,
            context.unavailable(ID, &[analysis_window::CONSTRUCT]),
        );
    };

    let force = context.trial.force();
    let last = end.saturating_sub(1);
    let lower_crossing =
        plateforce_core::rate::first_crossing_at_or_above(force, lower_newtons, start, last);
    // The upper level is searched forward from where the lower was reached, so the interval
    // is the one the rule names rather than the gap between two independent first crossings.
    let crossings = lower_crossing.and_then(|lower_crossing| {
        plateforce_core::rate::first_crossing_at_or_above(
            force,
            upper_newtons,
            lower_crossing.sample_index,
            last,
        )
        .map(|upper_crossing| (lower_crossing, upper_crossing))
    });

    let Some((lower_crossing, upper_crossing)) = crossings else {
        // Which of the two levels the trace never reached is the fact a reader acts on, so
        // the sentence names it and the value beside it.
        let (parameter, value) = match lower_crossing {
            Some(_) => (UPPER_PARAMETER, upper_newtons),
            None => (LOWER_PARAMETER, lower_newtons),
        };
        return DerivedOutcome::declined(
            bound,
            RuleRefusal::Refused(Box::new(plateforce_core::Refusal::value_not_accepted(
                ID,
                parameter,
                value,
                vec![format!(
                    "a level force reaches inside the analysis window, whose largest force is {:.1} N",
                    plateforce_core::peak::maximum_over(force, start, end).unwrap_or(f64::NAN)
                )],
            ))),
        );
    };

    let interval_seconds = (upper_crossing.position - lower_crossing.position)
        * context.trial.sample_interval_seconds();
    // A window that opens already above both levels reaches them at the same sample, and a
    // slope over an interval of zero is not a large rate, it is no measurement.
    if interval_seconds <= 0.0 {
        return DerivedOutcome::declined(
            bound,
            RuleRefusal::Refused(Box::new(plateforce_core::Refusal::span_selects_no_samples(
                ID,
                lower_crossing.sample_index,
                upper_crossing.sample_index,
            ))),
        );
    }

    // The interval the entry's rule text names first is the reciprocal expression of this
    // slope, and both levels are on the record as the values this rule read, so a reader
    // recovers it exactly from what the result already carries.
    DerivedOutcome {
        values: vec![(
            super::KEY,
            Some((upper_newtons - lower_newtons) / interval_seconds),
        )],
        placed: Vec::new(),
        bound,
        refusal: None,
    }
}
