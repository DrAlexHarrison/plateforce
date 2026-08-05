//! Where the positive lifting phase begins on a loaded lift.
//!
//! Two published answers, and they are not two searches: one reads the velocity sign change
//! off the trace, the other is placed by eye because its source states no algorithmic rule.
//! Both report the same key and let `computed_by` say which produced it, so a reader
//! comparing them holds the key still.
//!
//! The entry that predicts something testable is the velocity one: it says loaded-lift
//! mean-force disagreement across software is driven mostly by the end boundary rather than
//! by this one, which is the opposite of the countermovement-jump situation.

pub mod velocity_zero_crossing;
pub mod visual_inspection;

/// The construct id, as `registry/constructs.toml` declares it.
pub const CONSTRUCT: &str = "lifting_phase_start";

/// The key every rule here reports under.
pub const KEY: &str = "lifting_phase_start_seconds";

/// The sample a rule here placed, under the name later rules read it by.
pub const PLACED: &str = "lifting_phase_start";

/// Where the lift began, or nothing when no rule placed it.
pub fn placed(context: &crate::derived::DerivedContext) -> Option<usize> {
    context.sample(PLACED)
}
