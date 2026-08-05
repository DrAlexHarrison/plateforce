//! `norm.dimensionless_hof`: power divided by mass times gravity to the three-halves times leg
//! length to the one-half.
//!
//! A normalisation convention is exactly the kind of thing that silently breaks cross-study
//! comparison, and the reviewer who recommends this metric declines to restate the equation.
//! So the divisor is reported beside the scaled value rather than left inside the method's
//! name, and both the leg length and the gravity that formed it are on the record.
//!
//! Leg length is an anthropometric the plate cannot measure. The entry publishes no value for
//! it, because no representative leg length exists, and the rule declines by name until the
//! caller states one.

use crate::derived::{DerivedContext, DerivedOutcome, DerivedRule};
use crate::request::{MethodChoice, BODY_MASS_GLOBAL};
use crate::resolution::Resolution;
use crate::response::Quantity;
use crate::slots::power_peak;

pub const ID: &str = "norm.dimensionless_hof";

/// The entry's own name for the anthropometric, which it publishes no value for.
pub const LEG_LENGTH_PARAMETER: &str = "leg_length_meters";

pub const REQUIRED_NUMBERS: &[(&str, f64)] = &[(LEG_LENGTH_PARAMETER, 0.9)];
pub const REQUIRED_GLOBALS: &[(&str, f64)] = &[(BODY_MASS_GLOBAL, 52.0)];

pub const DIVISOR_KEY: &str = "hof_power_divisor_watts";
pub const KEY: &str = "peak_power_dimensionless";

pub const QUANTITIES: &[Quantity] = &[
    Quantity {
        key: DIVISOR_KEY,
        label: "What the power is divided by",
        unit: "watts",
        computed_by: Some(ID),
    },
    Quantity {
        key: KEY,
        label: "Peak power on Hof's dimensionless scale",
        unit: "dimensionless",
        computed_by: Some(ID),
    },
];

pub const RULE: DerivedRule = compute;

fn compute(
    context: &DerivedContext,
    choice: &MethodChoice,
    _warnings: &mut Vec<String>,
) -> DerivedOutcome {
    let mut resolved = Resolution::over(&choice.parameters, &choice.options, choice.claims());
    let leg_length_meters = resolved.required_number(ID, LEG_LENGTH_PARAMETER);
    let mass = super::body_mass_kilograms(context, &mut resolved, ID);
    let peak = super::measured(context, ID, power_peak::CONSTRUCT, power_peak::KEY);
    let bound = resolved.finish();

    let ((leg_length_meters, mass), (peak_watts, produced_by)) = match (leg_length_meters, mass, peak)
    {
        (Ok(length), Ok(mass), Ok(peak)) => ((length, mass), peak),
        (Err(refusal), _, _) | (_, Err(refusal), _) | (_, _, Err(refusal)) => {
            return DerivedOutcome::declined(bound, refusal)
        }
    };
    let Some(divisor) = plateforce_core::normalisation::dimensionless_power_divisor(
        mass,
        context.gravity_meters_per_second_squared,
        leg_length_meters,
    ) else {
        return DerivedOutcome::declined(
            bound,
            crate::resolution::RuleRefusal::Refused(Box::new(
                plateforce_core::Refusal::value_not_accepted(
                    ID,
                    LEG_LENGTH_PARAMETER,
                    leg_length_meters,
                    vec!["a leg length above zero".to_string()],
                ),
            )),
        );
    };
    super::rests_on(context, KEY, &produced_by);
    super::rests_on(context, DIVISOR_KEY, &produced_by);
    DerivedOutcome {
        values: vec![
            (DIVISOR_KEY, Some(divisor)),
            (KEY, Some(peak_watts / divisor)),
        ],
        placed: Vec::new(),
        bound,
        refusal: None,
    }
}
