//! `power.peak_from_height.regression`: peak power from a linear regression on jump height and
//! mass, with the coefficients of a named population.
//!
//! Ten sets, differing in their calibration sample rather than in what they claim power is.
//! Each is unbiased only on its own sample and biased by an unknown amount off it, so the
//! population is required and nothing is defaulted. They disagree with each other by hundreds
//! of watts on one jump, which is the demonstration this registry exists to make.
//!
//! The coefficients are registry data. The table below is the same data in the shape a rule
//! can read, and `the_coefficient_sets_are_the_ones_the_registry_publishes` holds the two
//! equal, so a set edited in the registry and not here is a failing test rather than a number
//! that quietly keeps its old calibration.

use plateforce_core::power::{HeightRegressionCoefficients, WATTS_PER_CENTIMETRE, WATTS_PER_METRE};

use crate::derived::{DerivedContext, DerivedOutcome, DerivedRule};
use crate::request::MethodChoice;
use crate::resolution::{Resolution, RuleRefusal};
use crate::response::Quantity;

pub const ID: &str = "power.peak_from_height.regression";

/// The entry's own name for the choice.
pub const POPULATION_PARAMETER: &str = "population";

/// The one name this rule cannot run without.
pub const REQUIRED_OPTIONS: &[(&str, &str)] = &[(POPULATION_PARAMETER, "harman1991")];

pub const GRAVITY_PARAMETER: &str = crate::slots::jh_takeoff_frame::flight_time::GRAVITY_PARAMETER;
pub const GRAVITY_DEFAULT_METERS_PER_SECOND_SQUARED: f64 =
    crate::slots::jh_takeoff_frame::flight_time::GRAVITY_DEFAULT_METERS_PER_SECOND_SQUARED;

/// The ten sets the registry publishes, keyed as it keys them.
///
/// Shetty's height term is per metre and the other nine are per centimetre. The unit travels
/// with the coefficient rather than being applied by whoever reads it, so the factor of a
/// hundred the registry warns about cannot be made here.
pub const COEFFICIENT_SETS: &[(&str, HeightRegressionCoefficients)] = &[
    (
        "harman1991",
        HeightRegressionCoefficients {
            jump_height_coefficient: 61.9,
            jump_height_unit: WATTS_PER_CENTIMETRE,
            body_mass_coefficient: 36.0,
            intercept_watts: -1822.0,
        },
    ),
    (
        "sayers1999_squat_jump",
        HeightRegressionCoefficients {
            jump_height_coefficient: 60.7,
            jump_height_unit: WATTS_PER_CENTIMETRE,
            body_mass_coefficient: 45.3,
            intercept_watts: -2055.0,
        },
    ),
    (
        "sayers1999_countermovement",
        HeightRegressionCoefficients {
            jump_height_coefficient: 51.9,
            jump_height_unit: WATTS_PER_CENTIMETRE,
            body_mass_coefficient: 48.9,
            intercept_watts: -2007.0,
        },
    ),
    (
        "shetty2002",
        HeightRegressionCoefficients {
            jump_height_coefficient: 1925.72,
            jump_height_unit: WATTS_PER_METRE,
            body_mass_coefficient: 14.74,
            intercept_watts: -666.3,
        },
    ),
    (
        "canavan2004",
        HeightRegressionCoefficients {
            jump_height_coefficient: 65.1,
            jump_height_unit: WATTS_PER_CENTIMETRE,
            body_mass_coefficient: 25.8,
            intercept_watts: -1413.1,
        },
    ),
    (
        "lara2006_male_sport_science",
        HeightRegressionCoefficients {
            jump_height_coefficient: 62.5,
            jump_height_unit: WATTS_PER_CENTIMETRE,
            body_mass_coefficient: 50.3,
            intercept_watts: -2184.7,
        },
    ),
    (
        "lara2006_female_elite_volleyball",
        HeightRegressionCoefficients {
            jump_height_coefficient: 83.1,
            jump_height_unit: WATTS_PER_CENTIMETRE,
            body_mass_coefficient: 42.0,
            intercept_watts: -2488.0,
        },
    ),
    (
        "lara2006_female_medium_volleyball",
        HeightRegressionCoefficients {
            jump_height_coefficient: 53.6,
            jump_height_unit: WATTS_PER_CENTIMETRE,
            body_mass_coefficient: 67.5,
            intercept_watts: -2624.1,
        },
    ),
    (
        "lara2006_female_sport_science",
        HeightRegressionCoefficients {
            jump_height_coefficient: 56.7,
            jump_height_unit: WATTS_PER_CENTIMETRE,
            body_mass_coefficient: 47.2,
            intercept_watts: -1772.6,
        },
    ),
    (
        "lara2006_female_university",
        HeightRegressionCoefficients {
            jump_height_coefficient: 68.2,
            jump_height_unit: WATTS_PER_CENTIMETRE,
            body_mass_coefficient: 40.8,
            intercept_watts: -1731.1,
        },
    ),
];

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
    let mut resolved = Resolution::over(
        &choice.parameters,
        &choice.options,
        choice.declared.of_entry(ID),
        choice.claims(),
    );
    let gravity = resolved.number_or_chosen(
        GRAVITY_PARAMETER,
        context.chosen_gravity_behind(super::KEY),
        GRAVITY_DEFAULT_METERS_PER_SECOND_SQUARED,
    );
    let population = resolved.required_enumerated(ID, POPULATION_PARAMETER, COEFFICIENT_SETS);
    let height = super::height_from_flight_meters(context, ID, gravity);
    let mass_kilograms = super::system_mass_kilograms(context, gravity);
    let bound = resolved.finish();

    let (coefficients, height_meters) = match (population, height) {
        (Ok(coefficients), Ok(height)) => (coefficients, height),
        (Err(refusal), _) | (_, Err(refusal)) => return DerivedOutcome::declined(bound, refusal),
    };

    match plateforce_core::power::peak_power_from_height_regression_watts(
        height_meters,
        mass_kilograms,
        &coefficients,
    ) {
        Some(watts) => DerivedOutcome {
            values: vec![(super::KEY, Some(watts))],
            placed: Vec::new(),
            bound,
            refusal: None,
        },
        None => DerivedOutcome::declined(
            bound,
            RuleRefusal::Refused(Box::new(plateforce_core::Refusal::value_not_accepted(
                ID,
                "jump_height_meters",
                height_meters,
                vec!["a finite height over a system of positive mass".to_string()],
            ))),
        ),
    }
}
