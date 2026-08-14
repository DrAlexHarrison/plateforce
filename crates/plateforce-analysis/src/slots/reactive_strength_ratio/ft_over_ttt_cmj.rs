//! `ratio.ft_over_ttt.cmj`: flight time over the time from onset to takeoff.
//!
//! The name is where the trouble is. The classical ratio is flight time over ground contact
//! time in a drop jump; a countermovement jump has no ground contact phase, so the same label
//! denotes different quantities in the two contexts. Three parties ship this under three
//! names, and the vendor label was the only thing separating the rows this entry was collapsed
//! from. So the entry states that label required and publishes no value for it, and this rule
//! refuses rather than filling one in: a ratio recorded under nobody's convention is the
//! collision the collapse was meant to make visible.
//!
//! The literature's own recommendation is against proliferation here. It correlates at 0.944
//! to 0.947 with the modified index with slightly worse between-day reliability, and the
//! source concludes there may be no requirement to report both.

use plateforce_core::{flight_time_seconds, time_to_takeoff_seconds, Refusal};

use crate::binding::{ONSET_CONSTRUCT, TAKEOFF_CONSTRUCT};
use crate::derived::{DerivedContext, DerivedOutcome, DerivedRule};
use crate::request::MethodChoice;
use crate::resolution::{Resolution, RuleRefusal};
use crate::response::Quantity;
use crate::slots::flight_time;

pub const ID: &str = "ratio.ft_over_ttt.cmj";

/// The party whose convention a number was produced under. Stated by the caller, because the
/// registry enumerates no set of them: the entry records that three parties ship this and
/// names none of them, and a list invented here would be this software publishing vendor
/// conventions no source states.
pub const VENDOR_PARAMETER: &str = "vendor_name";

/// What a probe of this rule states so it will run, for the checks that sweep every parameter
/// a rule needs before it produces. A placeholder rather than one of the three, because naming
/// one here would publish a convention the registry does not.
pub const REQUIRED_OPTIONS: &[(&str, &str)] = &[(VENDOR_PARAMETER, "unattributed")];

pub const QUANTITIES: &[Quantity] = &[Quantity {
    key: super::KEY,
    label: "Flight time to takeoff-time ratio",
    unit: "dimensionless",
    computed_by: Some(ID),
    produced_by_construct: None,
}];

pub const RULE: DerivedRule = compute;

fn compute(
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
    if resolved.stated_name(VENDOR_PARAMETER).is_none() {
        return DerivedOutcome::declined(
            resolved.finish(),
            RuleRefusal::Refused(Box::new(Refusal::required_parameter_unstated(
                ID,
                VENDOR_PARAMETER,
            ))),
        );
    }

    let Some(landmarks) = context.landmarks() else {
        return DerivedOutcome::declined(
            resolved.finish(),
            context.unavailable(ID, &[ONSET_CONSTRUCT, TAKEOFF_CONSTRUCT]),
        );
    };
    let Some(touchdown_index) = context.touchdown_index() else {
        return DerivedOutcome::declined(
            resolved.finish(),
            flight_time::no_landing_recorded(context, ID, landmarks.takeoff_index),
        );
    };

    let interval_seconds = context.trial.sample_interval_seconds();
    let flight = flight_time_seconds(landmarks.takeoff_index, touchdown_index, interval_seconds);
    let to_takeoff = time_to_takeoff_seconds(&landmarks, interval_seconds);

    // A non-positive denominator is a takeoff at or before the onset, which the landmark
    // bundle already refuses, so the guard here is over the quantity rather than over the
    // samples: a ratio reported from a zero interval is an infinity wearing a number's clothes.
    let value = (to_takeoff > 0.0).then(|| flight / to_takeoff);

    DerivedOutcome {
        values: vec![(super::KEY, value)],
        placed: Vec::new(),
        bound: resolved.finish(),
        refusal: None,
    }
}
