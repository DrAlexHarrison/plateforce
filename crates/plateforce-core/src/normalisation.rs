//! Expressing a quantity against the body that produced it.
//!
//! Ratio scaling and allometric scaling are one operation at two exponents, so they are one
//! function here and the exponent is the choice. Two laboratories scaling allometrically
//! with exponents fitted to their own samples have not computed the same quantity, which is
//! why the exponent travels with the number rather than sitting behind the method's name.
//!
//! Nothing here decides a method. A caller passes the exponent a bound rule resolved.

/// The quantity divided by body mass raised to the exponent.
///
/// An exponent of zero is the unmodified quantity and an exponent of one is ratio scaling,
/// so a caller needs no separate function for either.
pub fn scaled_by_body_mass(quantity: f64, body_mass_kilograms: f64, exponent: f64) -> Option<f64> {
    if !quantity.is_finite()
        || !body_mass_kilograms.is_finite()
        || body_mass_kilograms <= 0.0
        || !exponent.is_finite()
    {
        return None;
    }
    let scaled = quantity / body_mass_kilograms.powf(exponent);
    scaled.is_finite().then_some(scaled)
}

/// What a quantity is divided by when body mass is raised to an exponent.
///
/// Reported beside the scaled value because the units of the two differ with the exponent, so
/// a reader holding the scaled number alone cannot say what it is per.
pub fn body_mass_divisor(body_mass_kilograms: f64, exponent: f64) -> Option<f64> {
    scaled_by_body_mass(1.0, body_mass_kilograms, exponent).map(|reciprocal| 1.0 / reciprocal)
}

/// Hof's divisor for a power: mass times gravity to the three-halves times leg length to the
/// one-half.
///
/// Leg length is an anthropometric the plate cannot measure, so this takes it and the caller's
/// rule asks for it. The reviewer who recommends the metric declines to restate the equation,
/// which is the whole reason a normalisation convention has to travel with its number.
pub fn dimensionless_power_divisor(
    body_mass_kilograms: f64,
    gravity_meters_per_second_squared: f64,
    leg_length_meters: f64,
) -> Option<f64> {
    if body_mass_kilograms <= 0.0
        || gravity_meters_per_second_squared <= 0.0
        || leg_length_meters <= 0.0
    {
        return None;
    }
    let divisor = body_mass_kilograms
        * gravity_meters_per_second_squared.powf(1.5)
        * leg_length_meters.sqrt();
    divisor.is_finite().then_some(divisor)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MASS_KILOGRAMS: f64 = 61.5;
    const PEAK_FORCE_NEWTONS: f64 = 1482.0;

    #[test]
    fn an_exponent_of_zero_leaves_the_quantity_where_it_was() {
        assert_eq!(
            scaled_by_body_mass(PEAK_FORCE_NEWTONS, MASS_KILOGRAMS, 0.0),
            Some(PEAK_FORCE_NEWTONS)
        );
    }

    #[test]
    fn an_exponent_of_one_is_the_quantity_per_kilogram() {
        let scaled = scaled_by_body_mass(PEAK_FORCE_NEWTONS, MASS_KILOGRAMS, 1.0).unwrap();
        assert!((scaled - PEAK_FORCE_NEWTONS / MASS_KILOGRAMS).abs() < 1e-12);
    }

    /// The exponent is what the two published conventions differ by, so it has to move the
    /// number, and by a margin nobody could mistake for rounding.
    #[test]
    fn the_two_published_exponents_do_not_agree() {
        let two_thirds = scaled_by_body_mass(PEAK_FORCE_NEWTONS, MASS_KILOGRAMS, 0.67).unwrap();
        let ratio = scaled_by_body_mass(PEAK_FORCE_NEWTONS, MASS_KILOGRAMS, 1.0).unwrap();
        assert!(
            (two_thirds / ratio) > 3.0,
            "the exponent moved the number by a factor of {}, which a reader could take for \
             rounding",
            two_thirds / ratio
        );
    }

    /// The divisor and the scaling are one operation, so dividing by the one has to give the
    /// other rather than agreeing to a rounding.
    #[test]
    fn the_divisor_is_what_the_scaling_divided_by() {
        for exponent in [0.0f64, 0.67, 1.0] {
            let divisor = body_mass_divisor(MASS_KILOGRAMS, exponent).unwrap();
            let scaled = scaled_by_body_mass(PEAK_FORCE_NEWTONS, MASS_KILOGRAMS, exponent).unwrap();
            assert!(
                (PEAK_FORCE_NEWTONS / divisor - scaled).abs() < 1e-9,
                "{exponent}: {divisor} against {scaled}"
            );
        }
    }

    /// Gravity enters to the three-halves and leg length to the one-half, so a divisor that
    /// dropped either would move by a factor no reader could take for rounding.
    #[test]
    fn hofs_divisor_carries_gravity_and_leg_length_at_their_own_powers() {
        let divisor = dimensionless_power_divisor(MASS_KILOGRAMS, 9.81, 0.9).unwrap();
        let expected = MASS_KILOGRAMS * 9.81f64.powf(1.5) * 0.9f64.sqrt();
        assert!((divisor - expected).abs() < 1e-9);
        let longer = dimensionless_power_divisor(MASS_KILOGRAMS, 9.81, 1.1).unwrap();
        assert!(longer > divisor);
        assert!(dimensionless_power_divisor(MASS_KILOGRAMS, 9.81, 0.0).is_none());
        assert!(dimensionless_power_divisor(0.0, 9.81, 0.9).is_none());
    }

    #[test]
    fn a_mass_or_an_exponent_no_scaling_can_be_built_on_reports_nothing() {
        for (quantity, mass, exponent) in [
            (PEAK_FORCE_NEWTONS, 0.0, 0.67),
            (PEAK_FORCE_NEWTONS, -61.5, 0.67),
            (PEAK_FORCE_NEWTONS, f64::NAN, 0.67),
            (PEAK_FORCE_NEWTONS, MASS_KILOGRAMS, f64::INFINITY),
            (f64::NAN, MASS_KILOGRAMS, 0.67),
        ] {
            assert!(
                scaled_by_body_mass(quantity, mass, exponent).is_none(),
                "quantity {quantity} mass {mass} exponent {exponent} produced a value"
            );
        }
    }
}
