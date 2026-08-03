//! Where propulsion ends, which one school puts before takeoff and another puts at it.
//!
//! Peak velocity necessarily precedes takeoff, because velocity peaks where net force crosses
//! zero while takeoff is declared later, so propulsion duration is systematically shorter and
//! mean propulsion force systematically higher under the rule here than under the reading that
//! propulsion ends when the athlete leaves the plate. Both appear in the literature and
//! neither acknowledges the other.

pub mod peak_com_velocity;

/// The construct id, as `registry/constructs.toml` declares it.
pub const CONSTRUCT: &str = "propulsion_phase_end";

/// The sample a propulsion-end rule placed, under the name later rules read it by.
pub const PLACED: &str = "propulsion_phase_end";

pub const KEY: &str = "propulsion_phase_end_seconds";

/// The boundary a rule needs, or nothing when no rule placed one.
pub fn placed(context: &crate::derived::DerivedContext) -> Option<usize> {
    context.sample(PLACED)
}
