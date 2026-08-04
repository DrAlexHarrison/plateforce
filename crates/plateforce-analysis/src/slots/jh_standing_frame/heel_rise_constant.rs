//! `jumpheight.standing.flight_time_anthropometric_correction.wade2020`: a flight-time height
//! plus the rise it cannot see.
//!
//! The flight-time height measures from the instant of takeoff, at which the ankle is already
//! plantarflexed, so it omits the climb from quiet standing to that instant. Wade, Lichtwark
//! and Farris 2020 stand an anthropometric constant in for that climb, which converts a
//! flight-time number into a standing-frame one without a second instrument. On their cohort
//! it took the gap against the marker-based reference from 9.6 cm to 0.4 cm.
//!
//! The constant treats heel lift as stable within a subject.
//! `jumpheight.flight_time.ankle_angle_corrected` measures the posture on every trial instead,
//! on the opposite premise. The published evidence cuts both ways, both entries are registered,
//! and a reader picks.

use plateforce_core::{heel_rise_constant_meters, jump_height_from_flight_time, Refusal};

use crate::binding::TAKEOFF_CONSTRUCT;
use crate::derived::{DerivedContext, DerivedOutcome, DerivedRule};
use crate::request::MethodChoice;
use crate::resolution::{Resolution, RuleRefusal};
use crate::response::Quantity;
use crate::slots::flight_time;

pub const ID: &str = "jumpheight.standing.flight_time_anthropometric_correction.wade2020";

/// The sine of the takeoff foot angle, being sin(61.4 degrees). A single-cohort mean with a
/// 4.8 degree standard deviation that the rule treats as fixed, which is the entry's own
/// account of it.
pub const FOOT_ANGLE_SINE_PARAMETER: &str = "foot_angle_sine";
pub const FOOT_ANGLE_SINE_DEFAULT: f64 = 0.88;

/// The length the sine multiplies. The source's printed formula names it foot length and its
/// own text defines it as the distance from the medial malleolus to the toes, so the rule takes
/// the malleolus-to-toe distance where a reader measured it and scales foot length where they
/// did not.
pub const MALLEOLUS_TO_TOE_PARAMETER: &str = "malleolus_to_toe_length_m";
pub const FOOT_LENGTH_PARAMETER: &str = "foot_length_m";
pub const MALLEOLUS_FRACTION_PARAMETER: &str = "malleolus_fraction_of_foot_length";
pub const MALLEOLUS_FRACTION_DEFAULT: f64 = 0.787;

/// Zero for a barefoot jump, which is a measurement rather than an absence, and the reason the
/// name is read and refused rather than defaulted: a shoe lifts heel and toe by different
/// amounts and the source includes the term for exactly that.
pub const SOLE_THICKNESS_PARAMETER: &str = "sole_thickness_m";

/// Ground to lateral malleolus in quiet standing, which the constant subtracts because the
/// ankle was already that high before the athlete moved.
pub const ANKLE_HEIGHT_PARAMETER: &str = "ankle_height_m";

pub const QUANTITIES: &[Quantity] = &[Quantity {
    key: super::KEY,
    label: "Jump height, standing frame",
    unit: "meters",
    computed_by: Some(ID),
}];

pub const RULE: DerivedRule = compute;

fn compute(
    context: &DerivedContext,
    choice: &MethodChoice,
    _warnings: &mut Vec<String>,
) -> DerivedOutcome {
    let mut resolved = Resolution::over(&choice.parameters, &choice.options, choice.claims());

    let foot_angle_sine = resolved.number(FOOT_ANGLE_SINE_PARAMETER, FOOT_ANGLE_SINE_DEFAULT);
    let malleolus_fraction =
        resolved.number(MALLEOLUS_FRACTION_PARAMETER, MALLEOLUS_FRACTION_DEFAULT);
    let measured_length = resolved.stated(MALLEOLUS_TO_TOE_PARAMETER);
    let malleolus_to_toe_meters = match measured_length {
        Some(length) => length,
        None => match resolved.required_number(ID, FOOT_LENGTH_PARAMETER) {
            Ok(foot_length) => foot_length * malleolus_fraction,
            Err(refusal) => return DerivedOutcome::declined(resolved.finish(), refusal),
        },
    };
    let sole_thickness = match resolved.required_number(ID, SOLE_THICKNESS_PARAMETER) {
        Ok(value) => value,
        Err(refusal) => return DerivedOutcome::declined(resolved.finish(), refusal),
    };
    let ankle_height = match resolved.required_number(ID, ANKLE_HEIGHT_PARAMETER) {
        Ok(value) => value,
        Err(refusal) => return DerivedOutcome::declined(resolved.finish(), refusal),
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
            RuleRefusal::Refused(Box::new(Refusal::required_parameter_unstated(
                ID,
                flight_time::TOUCHDOWN_FIELD,
            ))),
        );
    };

    let gravity = context.gravity_meters_per_second_squared;
    let heel_rise = heel_rise_constant_meters(
        malleolus_to_toe_meters,
        sole_thickness,
        ankle_height,
        foot_angle_sine,
    );
    DerivedOutcome {
        values: vec![(
            super::KEY,
            Some(jump_height_from_flight_time(seconds, gravity) + heel_rise),
        )],
        placed: Vec::new(),
        bound: resolved.finish(),
        refusal: None,
    }
}
