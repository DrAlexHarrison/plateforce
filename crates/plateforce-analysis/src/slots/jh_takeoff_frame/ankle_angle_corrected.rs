//! `jumpheight.flight_time.ankle_angle_corrected`: the projectile equation with the landing
//! posture taken out of it.
//!
//! The projectile equation assumes the centre of mass is at the same height at landing as at
//! takeoff. An athlete who takes off plantarflexed and lands with the foot flat is lower at
//! landing than at takeoff, the fall is longer than the rise, and the extra flight time reads
//! as height. Goncalves, Baptista, Tufano, Blazevich and Vieira 2024 measure the ankle at both
//! instants, convert the posture change into a centre-of-mass offset, and take the height from
//! the ascent alone.
//!
//! The correction is not a rounding. Uncorrected, the error reaches 59.6 percent of the number
//! on a 0.10 m jump by a 1.98 m subject landing flat, and 8 to 13 percent on an average 0.30 m
//! jump, which is why the entry forces a decision on low and loaded jumps.
//!
//! It disagrees with `jumpheight.standing.flight_time_anthropometric_correction.wade2020` on
//! an empirical question neither has settled: whether heel lift is stable enough within a
//! subject to be a constant. Both are registered and both run.

use plateforce_core::{
    ankle_to_toe_segment, ankle_to_toe_standing_angle_degrees,
    jump_height_from_flight_time_with_landing_offset,
    landing_below_takeoff_from_ankle_angles_meters, AnkleToToeSegment, Refusal,
};

use crate::binding::TAKEOFF_CONSTRUCT;
use crate::derived::{DerivedContext, DerivedOutcome, DerivedRule};
use crate::request::MethodChoice;
use crate::resolution::{Resolution, RuleRefusal};
use crate::response::Quantity;
use crate::slots::flight_time;

pub const ID: &str = "jumpheight.flight_time.ankle_angle_corrected";

/// The two postures the correction is the difference between, in degrees of plantarflexion
/// away from a flat foot. Required, because the whole rule is the change between them and a
/// filled-in value would report an uncorrected height under a corrected rule's name.
pub const TAKEOFF_ANGLE_PARAMETER: &str = "ankle_angle_at_takeoff_degrees";
pub const LANDING_ANGLE_PARAMETER: &str = "ankle_angle_at_landing_degrees";

/// The segment those angles rotate. Measured directly where the reader has it, which the
/// source prefers, and estimated from stature otherwise.
pub const ANKLE_TO_TOE_PARAMETER: &str = "ankle_to_toe_length_m";
pub const STATURE_PARAMETER: &str = "stature_m";

/// The three fractions the published length equation scales stature by. The entry publishes
/// the foot-length and malleolus fractions and omits the ankle-height one, which is the
/// fraction that makes the segment 0.126 of stature rather than 0.120.
pub const ANKLE_HEIGHT_FRACTION_PARAMETER: &str = "ankle_height_fraction_of_stature";
pub const FOOT_LENGTH_FRACTION_PARAMETER: &str = "foot_length_fraction_of_stature";
pub const MALLEOLUS_FRACTION_PARAMETER: &str = "malleolus_fraction_of_foot_length";
pub const ANKLE_HEIGHT_FRACTION_DEFAULT: f64 = 0.039;
pub const FOOT_LENGTH_FRACTION_DEFAULT: f64 = 0.152;
pub const MALLEOLUS_FRACTION_DEFAULT: f64 = 0.787;

pub const QUANTITIES: &[Quantity] = &[Quantity {
    key: super::KEY,
    label: "Jump height, takeoff frame",
    unit: "meters",
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

    // The three fractions are read whether or not the segment is measured directly, because
    // the standing lean of the segment comes from the same triangle and the correction needs
    // it either way.
    let ankle_height_fraction = resolved.number(
        ANKLE_HEIGHT_FRACTION_PARAMETER,
        ANKLE_HEIGHT_FRACTION_DEFAULT,
    );
    let foot_length_fraction =
        resolved.number(FOOT_LENGTH_FRACTION_PARAMETER, FOOT_LENGTH_FRACTION_DEFAULT);
    let malleolus_fraction =
        resolved.number(MALLEOLUS_FRACTION_PARAMETER, MALLEOLUS_FRACTION_DEFAULT);
    let stature_meters = resolved.stated(STATURE_PARAMETER);
    let measured_segment = resolved.stated(ANKLE_TO_TOE_PARAMETER);

    let takeoff_angle = match resolved.required_number(ID, TAKEOFF_ANGLE_PARAMETER) {
        Ok(value) => value,
        Err(refusal) => return DerivedOutcome::declined(resolved.finish(), refusal),
    };
    let landing_angle = match resolved.required_number(ID, LANDING_ANGLE_PARAMETER) {
        Ok(value) => value,
        Err(refusal) => return DerivedOutcome::declined(resolved.finish(), refusal),
    };

    // Stature is what the refusal names when neither length is stated: it is the measurement a
    // reader is likelier to hold, and the source offers it as the route to the other one.
    let segment = match (measured_segment, stature_meters) {
        (Some(length_meters), _) => AnkleToToeSegment {
            length_meters,
            standing_angle_degrees: ankle_to_toe_standing_angle_degrees(
                ankle_height_fraction,
                foot_length_fraction,
                malleolus_fraction,
            ),
        },
        (None, Some(stature)) => ankle_to_toe_segment(
            stature,
            ankle_height_fraction,
            foot_length_fraction,
            malleolus_fraction,
        ),
        (None, None) => {
            return DerivedOutcome::declined(
                resolved.finish(),
                RuleRefusal::Refused(Box::new(Refusal::required_parameter_unstated(
                    ID,
                    STATURE_PARAMETER,
                ))),
            )
        }
    };

    let Some(takeoff_index) = context.takeoff_index() else {
        return DerivedOutcome::declined(
            resolved.finish(),
            context.unavailable(ID, &[TAKEOFF_CONSTRUCT]),
        );
    };
    let Some(seconds) = flight_time::seconds(context, takeoff_index) else {
        return DerivedOutcome::declined(
            resolved.finish(),
            flight_time::no_landing_recorded(context, ID, takeoff_index),
        );
    };

    let landing_below_takeoff_meters =
        landing_below_takeoff_from_ankle_angles_meters(segment, takeoff_angle, landing_angle);
    DerivedOutcome {
        values: vec![(
            super::KEY,
            Some(jump_height_from_flight_time_with_landing_offset(
                seconds,
                landing_below_takeoff_meters,
                context.gravity_behind(Some(super::KEY)),
            )),
        )],
        placed: Vec::new(),
        bound: resolved.finish(),
        refusal: None,
    }
}

/// The measurements this rule declines without, each with a plausible adult value.
///
/// The two angles are the rule's whole content and the registry publishes no default for
/// either, so it refuses rather than inventing a posture. Stature answers the refusal the
/// segment length otherwise raises. Held here so a surface offering the rule knows what to
/// ask for, and a check reaching it gets past the refusal to the control it is probing.
pub const REQUIRED_NUMBERS: &[(&str, f64)] = &[
    (TAKEOFF_ANGLE_PARAMETER, 25.0),
    (LANDING_ANGLE_PARAMETER, 10.0),
    (STATURE_PARAMETER, 1.70),
];
