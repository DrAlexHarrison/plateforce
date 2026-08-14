//! The interval the athlete was off the plate.
//!
//! One rule, and the two samples that bound it come from elsewhere: takeoff from the bound
//! takeoff rule, and the return to the plate from the request. Neither is this construct's to
//! place, which is why the interval is an entry of its own rather than a line inside the
//! heights that divide by it.

pub mod takeoff_to_touchdown;

use plateforce_core::Refusal;

use crate::derived::DerivedContext;
use crate::resolution::RuleRefusal;

/// The construct id, as `registry/constructs.toml` declares it.
pub const CONSTRUCT: &str = "flight_time";

pub const KEY: &str = "flight_time_seconds";

/// The request field carrying the sample the athlete came back down on. The analysis-wide
/// name, so the refusal below and the value a stated landing is recorded under cannot drift
/// into two spellings of one field.
pub const TOUCHDOWN_FIELD: &str = crate::request::TOUCHDOWN_GLOBAL;

/// The interval from the placed takeoff to the return to the plate, or nothing where the
/// request stated no return.
///
/// `Landmarks` fills an unstated touchdown with the last sample of the recording, which is the
/// right reading for a window and the wrong one here: it would report every sample after
/// takeoff as flight, on a recording that runs for a minute as readily as on one that stops at
/// the landing. So the placed touchdown is what this reads, and it takes the two samples one
/// at a time rather than as the three-landmark bundle, because the onset is not one of them.
pub fn seconds(context: &DerivedContext, takeoff_index: usize) -> Option<f64> {
    let touchdown_index = context.touchdown_index()?;
    Some(plateforce_core::flight_time_seconds(
        takeoff_index,
        touchdown_index,
        context.trial.sample_interval_seconds(),
    ))
}

/// What the four rules bounded by the return to the plate report where the recording holds no
/// return.
///
/// One sentence for the four, so a reader who asked for flight time, either height taken from
/// it, or the ratio over time to takeoff meets one account of the recording rather than four.
/// It counts the samples the reading was taken over, which is the denominator behind the
/// claim: a recording holding four samples past takeoff and one holding four thousand are
/// different evidence for the same sentence.
pub fn no_landing_recorded(
    context: &DerivedContext,
    method_id: &str,
    takeoff_index: usize,
) -> RuleRefusal {
    RuleRefusal::Refused(Box::new(Refusal::landing_not_in_recording(
        method_id,
        crate::boundaries::LANDING_CONSTRUCT,
        context.trial.len().saturating_sub(takeoff_index + 1),
    )))
}
