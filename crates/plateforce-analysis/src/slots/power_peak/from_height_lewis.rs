//! `power.peak_from_height.lewis`: peak power estimated dimensionally from jump height and
//! mass, with no power series formed at all.
//!
//! The source it was transcribed from carries a 29-line comment on the formula's own
//! provenance: first published in a 1974 interval-training book with no reference beyond a
//! courtesy line, and one author later confirmed by telephone that he developed it with a
//! student. The original treated kilograms as force and the multiplier that fixes that is a
//! later correction. It substantially underestimates the measured peak, which the registry
//! records against it.
//!
//! It reports under the same key as the rule that reads the series, because it answers the
//! same question from worse data rather than a different question.

use crate::derived::{DerivedContext, DerivedOutcome, DerivedRule};
use crate::request::MethodChoice;
use crate::resolution::{Resolution, RuleRefusal};
use crate::response::Quantity;

pub const ID: &str = "power.peak_from_height.lewis";

/// The name the flight-time height entry publishes for gravity, and the value it declares.
///
/// Read through the same name the height entry uses, because the height this estimate is built
/// on is that entry's number and a second constant here would give a height the entry it names
/// does not produce.
pub const GRAVITY_PARAMETER: &str =
    crate::slots::jh_takeoff_frame::flight_time::GRAVITY_PARAMETER;
pub const GRAVITY_DEFAULT_METERS_PER_SECOND_SQUARED: f64 =
    crate::slots::jh_takeoff_frame::flight_time::GRAVITY_DEFAULT_METERS_PER_SECOND_SQUARED;

pub const QUANTITIES: &[Quantity] = &[Quantity {
    key: super::KEY,
    label: "Peak power",
    unit: "watts",
    computed_by: Some(ID),
}];

pub const RULE: DerivedRule = compute;

fn compute(
    context: &DerivedContext,
    choice: &MethodChoice,
    _warnings: &mut Vec<String>,
) -> DerivedOutcome {
    let mut resolved = Resolution::over(&choice.parameters, &choice.options, choice.claims());
    let gravity = resolved.number_or_chosen(
        GRAVITY_PARAMETER,
        context.chosen_gravity(),
        GRAVITY_DEFAULT_METERS_PER_SECOND_SQUARED,
    );
    let height = super::height_from_flight_meters(context, ID, gravity);
    let mass_kilograms = super::system_mass_kilograms(context, gravity);
    let bound = resolved.finish();

    let height_meters = match height {
        Ok(height) => height,
        Err(refusal) => return DerivedOutcome::declined(bound, refusal),
    };

    match plateforce_core::power::peak_power_from_height_lewis_watts(height_meters, mass_kilograms)
    {
        Some(watts) => DerivedOutcome {
            values: vec![(super::KEY, Some(watts))],
            placed: Vec::new(),
            bound,
            refusal: None,
        },
        // A descending flight or a weightless system is the recording rather than the
        // estimate, and the height is the number a reader looks at to see why.
        None => DerivedOutcome::declined(
            bound,
            RuleRefusal::Refused(Box::new(plateforce_core::Refusal::value_not_accepted(
                ID,
                "jump_height_meters",
                height_meters,
                vec!["a height at or above zero, over a system of positive mass".to_string()],
            ))),
        ),
    }
}
