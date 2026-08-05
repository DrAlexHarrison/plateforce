//! The largest power reached, under three rules that reach it from different data.
//!
//! One reads the power series the plate produced. Two never form a series at all: they
//! estimate the peak from a jump height and a mass, one dimensionally and one from a
//! regression fitted to a named population. The registry files ten coefficient sets for the
//! second and they disagree with each other by hundreds of watts on the same jump, which is
//! why the population is required rather than defaulted.
//!
//! All three report the peak under one key and let `computed_by` say which produced it, so
//! the choice moves a value rather than settling which values exist.

pub mod from_height_lewis;
pub mod from_height_regression;
pub mod instantaneous;

use plateforce_core::Refusal;

use crate::derived::DerivedContext;
use crate::resolution::RuleRefusal;
use crate::slots::flight_time;

/// The construct id, as `registry/constructs.toml` declares it.
pub const CONSTRUCT: &str = "mechanical_power.peak";

/// The key all three rules report under.
pub const KEY: &str = "peak_power_watts";

/// The entry the two height-based estimates take their height from.
///
/// Neither publishes a jump-height rule of its own, and both were read off an implementation
/// that measures height from flight time, so that is the height they run on and the entry is
/// named among the entries their number rests on. A height rule chosen elsewhere in the
/// request does not reach them, and a number that quietly took whichever height the caller
/// happened to select would be an estimate whose input nobody could name.
pub const HEIGHT_ENTRY: &str = "jumpheight.takeoff.flight_time";

/// The jump height the two estimates read, from the flight the recording holds.
///
/// The gravity is the one chosen for the analysis, and it is recorded by the caller of this
/// through the same name the flight-time height entry publishes, so the two agree on the
/// constant as well as on the equation.
pub(crate) fn height_from_flight_meters(
    context: &DerivedContext,
    method_id: &str,
    gravity_meters_per_second_squared: f64,
) -> Result<f64, RuleRefusal> {
    let Some(takeoff_index) = context.takeoff_index() else {
        return Err(context.unavailable(method_id, &[crate::binding::TAKEOFF_CONSTRUCT]));
    };
    let Some(seconds) = flight_time::seconds(context, takeoff_index) else {
        return Err(RuleRefusal::Refused(Box::new(
            Refusal::required_parameter_unstated(method_id, flight_time::TOUCHDOWN_FIELD),
        )));
    };
    context.rests_on(KEY, &[HEIGHT_ENTRY]);
    Ok(plateforce_core::jump_height_from_flight_time(
        seconds,
        gravity_meters_per_second_squared,
    ))
}

/// The mass the two estimates multiply, which is the weighed system rather than the athlete.
///
/// Both entries state the sum of body mass and any external load, and that is exactly what the
/// plate weighed during the weighing epoch. Dividing the weighed force by the athlete's own
/// stated mass instead would report a bar as part of the athlete.
pub(crate) fn system_mass_kilograms(context: &DerivedContext, gravity: f64) -> f64 {
    context.epoch().system_weight_newtons / gravity
}
