//! The interval the athlete was off the plate.
//!
//! One rule, and the two samples that bound it come from elsewhere: takeoff from the bound
//! takeoff rule, and the return to the plate from the request. Neither is this construct's to
//! place, which is why the interval is an entry of its own rather than a line inside the
//! heights that divide by it.

pub mod takeoff_to_touchdown;

use plateforce_core::Landmarks;

use crate::derived::DerivedContext;

/// The construct id, as `registry/constructs.toml` declares it.
pub const CONSTRUCT: &str = "flight_time";

pub const KEY: &str = "flight_time_seconds";

/// The request field carrying the sample the athlete came back down on.
pub const TOUCHDOWN_FIELD: &str = "touchdown_index";

/// The interval from the placed takeoff to the return to the plate, or nothing where the
/// request stated no return.
///
/// `Landmarks` fills an unstated touchdown with the last sample of the recording, which is the
/// right reading for a window and the wrong one here: it would report every sample after
/// takeoff as flight, on a recording that runs for a minute as readily as on one that stops at
/// the landing. So the request's own field is what this reads.
pub fn seconds(context: &DerivedContext, landmarks: &Landmarks) -> Option<f64> {
    context.touchdown_index?;
    Some(plateforce_core::flight_time_seconds(
        landmarks,
        context.trial.sample_interval_seconds(),
    ))
}
