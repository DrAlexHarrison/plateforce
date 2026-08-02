//! Absolute vertical ground reaction force from a plate that was zeroed under load.
//!
//! A plate zeroed while the athlete already stood on it reports their weight as nothing, so
//! every quantity taken from it is out by one bodyweight and the trace looks ordinary.
//!
//! Nothing here decides a method. A caller passes the ceiling a bound rule resolved and the
//! mass a caller declared, and turns an absent finding into a refusal under the id it bound.

/// What the quiet-standing window says about how the plate was zeroed.
///
/// Both sides of the comparison travel, because a reader deciding whether to trust a
/// reconstruction needs to see how far the window sat from zero, and a window at 3 N and one
/// at 300 N are different facts about the recording.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TareFinding {
    pub quiet_epoch_mean_newtons: f64,
    pub ceiling_newtons: f64,
}

impl TareFinding {
    /// A window carrying a standing athlete reports their weight. One reporting nothing was
    /// zeroed while they stood on it.
    pub fn plate_was_tared(&self) -> bool {
        self.quiet_epoch_mean_newtons.abs() <= self.ceiling_newtons
    }

    /// How far the window sits from the ceiling it was judged against. Negative when the
    /// plate reads a weight, so a caller can report how clear the call was.
    pub fn margin_newtons(&self) -> f64 {
        self.ceiling_newtons - self.quiet_epoch_mean_newtons.abs()
    }
}

/// Nothing when either side of the comparison is not a number the other can be judged
/// against, which is a failure to check rather than a plate that was not tared.
pub fn tare_finding(quiet_epoch_mean_newtons: f64, ceiling_newtons: f64) -> Option<TareFinding> {
    (quiet_epoch_mean_newtons.is_finite() && ceiling_newtons.is_finite() && ceiling_newtons >= 0.0)
        .then_some(TareFinding {
            quiet_epoch_mean_newtons,
            ceiling_newtons,
        })
}

/// Absolute force from a tared recording: measured force plus the weight of a declared mass.
///
/// Nothing when the declared mass is not a positive number, because a mass a caller could
/// not have meant would shift every sample by a quantity nobody chose.
pub fn reconstruct_absolute_force(
    measured_force_newtons: &[f64],
    declared_mass_kilograms: f64,
    gravity_meters_per_second_squared: f64,
) -> Option<Vec<f64>> {
    let weight_newtons =
        declared_weight_newtons(declared_mass_kilograms, gravity_meters_per_second_squared)?;
    Some(
        measured_force_newtons
            .iter()
            .map(|measured| measured + weight_newtons)
            .collect(),
    )
}

/// The constant a tared recording is short by.
pub fn declared_weight_newtons(
    declared_mass_kilograms: f64,
    gravity_meters_per_second_squared: f64,
) -> Option<f64> {
    (declared_mass_kilograms.is_finite()
        && declared_mass_kilograms > 0.0
        && gravity_meters_per_second_squared.is_finite()
        && gravity_meters_per_second_squared > 0.0)
        .then(|| declared_mass_kilograms * gravity_meters_per_second_squared)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::statistics::mean;
    use crate::STANDARD_GRAVITY_METERS_PER_SECOND_SQUARED;

    const CEILING_NEWTONS: f64 = 20.0;
    const MASS_KILOGRAMS: f64 = 61.5;

    fn standing() -> Vec<f64> {
        (0..1200)
            .map(|index| {
                MASS_KILOGRAMS * STANDARD_GRAVITY_METERS_PER_SECOND_SQUARED
                    + ((index % 17) as f64 - 8.0) * 0.4
            })
            .collect()
    }

    fn tared() -> Vec<f64> {
        let weight = MASS_KILOGRAMS * STANDARD_GRAVITY_METERS_PER_SECOND_SQUARED;
        standing().iter().map(|force| force - weight).collect()
    }

    #[test]
    fn a_window_reading_nothing_was_zeroed_under_the_athlete_and_one_reading_a_weight_was_not() {
        let tared_mean = mean(&tared()).unwrap();
        let standing_mean = mean(&standing()).unwrap();

        assert!(tare_finding(tared_mean, CEILING_NEWTONS)
            .unwrap()
            .plate_was_tared());
        assert!(!tare_finding(standing_mean, CEILING_NEWTONS)
            .unwrap()
            .plate_was_tared());
    }

    /// The reconstruction has to move the quiet window by the whole declared weight. An
    /// implementation that returned the trace unchanged would leave it reading nothing,
    /// which is the state the rule exists to leave behind.
    #[test]
    fn reconstruction_puts_the_athletes_weight_back_under_them() {
        let expected = mean(&standing()).unwrap();
        let reconstructed = reconstruct_absolute_force(
            &tared(),
            MASS_KILOGRAMS,
            STANDARD_GRAVITY_METERS_PER_SECOND_SQUARED,
        )
        .unwrap();
        let recovered = mean(&reconstructed).unwrap();

        assert!(
            (recovered - expected).abs() < 1e-9,
            "reconstructed quiet standing reads {recovered} N where the plate would have read {expected} N"
        );
        assert!(
            !tare_finding(recovered, CEILING_NEWTONS)
                .unwrap()
                .plate_was_tared(),
            "the reconstructed trace still reads as tared, so the weight was not put back"
        );
    }

    #[test]
    fn a_mass_nobody_could_have_meant_reconstructs_nothing() {
        for mass in [0.0, -61.5, f64::NAN, f64::INFINITY] {
            assert!(
                reconstruct_absolute_force(
                    &tared(),
                    mass,
                    STANDARD_GRAVITY_METERS_PER_SECOND_SQUARED
                )
                .is_none(),
                "{mass} kg produced a reconstruction"
            );
        }
    }

    /// The margin is what a reader is shown beside the verdict, so it has to carry the sign
    /// that says which way the call went.
    #[test]
    fn the_margin_says_how_clear_the_call_was() {
        let tared_finding = tare_finding(mean(&tared()).unwrap(), CEILING_NEWTONS).unwrap();
        let standing_finding = tare_finding(mean(&standing()).unwrap(), CEILING_NEWTONS).unwrap();

        assert!(tared_finding.margin_newtons() > 0.0);
        assert!(standing_finding.margin_newtons() < 0.0);
    }
}
