//! Force added up over a stretch of time measured from onset.
//!
//! More reliable than a rate at the same epoch and, on the source that files both, more
//! functionally relevant: an integral is robust to boundary placement where a derivative is
//! not, which is the same structure as jump height against time to takeoff. Applied isometric
//! data agree, CV 4.3 to 8.7 percent against the rate's, so a practitioner is usually better
//! served by the impulse and the interface should not bury it.
//!
//! Whether system weight comes out before integrating is `impulse.convention`, a registry
//! entry of its own with a required value and no published default. Both rules here read it,
//! record it, and name that entry among the entries their number rests on, because a 50 ms
//! epoch from a static start under the gross convention is dominated by system weight times
//! the epoch: a large part of what it measures is how heavy the athlete is.

pub mod epoch_from_onset;
pub mod to_fraction_of_peak;

use crate::resolution::{Resolution, RuleRefusal};

/// The construct id, as `registry/constructs.toml` declares it.
pub const CONSTRUCT: &str = "epoch_impulse";

/// The key both rules report under, so they are two answers to one question.
pub const KEY: &str = "epoch_impulse_newton_seconds";

/// The entry that owns the convention, and its two published values.
pub const CONVENTION_ENTRY: &str = "impulse.convention";
pub const CONVENTION_PARAMETER: &str = "convention";
pub const NET: &str = "net";
pub const GROSS: &str = "gross";

/// What comes off the force before it is integrated: nothing, or the whole system weight.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Convention {
    Net,
    Gross,
}

/// The offset the integral removes at every sample, which is the whole of the difference
/// between the two conventions.
///
/// The weighing epoch is read on the net branch alone, so a gross impulse does not name the
/// weighing rule among the rules its number rests on. It did not read one.
pub(crate) fn offset_newtons(
    context: &crate::derived::DerivedContext,
    convention: Convention,
) -> f64 {
    match convention {
        Convention::Net => context.epoch().system_weight_newtons,
        Convention::Gross => 0.0,
    }
}

/// The convention the caller stated, refused rather than filled where they stated none.
///
/// Universal for jumps and in wide use for dead-start lifts are not one practice with a
/// majority, they are two, and the entry publishes no default for that reason.
pub(crate) fn convention(
    resolved: &mut Resolution,
    method_id: &str,
) -> Result<Convention, RuleRefusal> {
    resolved.required_enumerated(
        method_id,
        CONVENTION_PARAMETER,
        &[(NET, Convention::Net), (GROSS, Convention::Gross)],
    )
}

/// The value a rule read, written onto the record under the entry that owns the choice.
///
/// A rule declining before it reached the trace still consulted the name, and this is called
/// on the answering path, where the value is settled and the entry can be named beside the
/// number it moved.
pub(crate) fn record_entry_behind(
    context: &crate::derived::DerivedContext,
    quantity_key: &'static str,
) {
    context.rests_on(quantity_key, &[CONVENTION_ENTRY]);
}
