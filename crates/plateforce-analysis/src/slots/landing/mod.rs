//! The instant the athlete is back on the plate, and the two published claims about it.
//!
//! Both rules find a rising crossing of a force threshold. They differ in which threshold, and
//! in how long the return has to hold: one reads the threshold the takeoff rule resolved and
//! asks for no span, the other takes a threshold stated for the rising edge and a span stated
//! beside it. Tying the two edges makes a threshold error compound, because a higher threshold
//! places takeoff earlier and landing later and flight time grows at both ends; stating the
//! rising edge separately lets them differ, and one surveyed tool sets them a factor of
//! seventeen apart in persistence.
//!
//! Flight-time jump height goes as the square of flight time, so the choice between them
//! reaches a headline number. A flight time is comparable with another only when the rule
//! behind both edges is named, which is why the tied rule is recorded even though it is the
//! one that runs when nobody states otherwise.

pub mod absolute_force;
pub mod tied_to_takeoff;

/// The construct id, as `registry/constructs.toml` declares it.
pub const CONSTRUCT: &str = "landing";

/// The sample a landing rule placed, under the name later rules read it by.
pub const PLACED: &str = "landing";

/// The key both rules report under. Holding the key still and letting `computed_by` vary is
/// what makes them two answers to one question rather than two quantities.
pub const KEY: &str = "landing_seconds";

/// The threshold the rising edge is compared against, on the rule that states its own.
pub const THRESHOLD_PARAMETER: &str = "threshold_n";

/// How long the return has to hold above that threshold.
pub const PERSISTENCE_PARAMETER: &str = "persistence_ms";

/// The landing a rule needs, or nothing when no landing rule placed one.
pub fn placed(context: &crate::derived::DerivedContext) -> Option<usize> {
    context.sample(PLACED)
}
