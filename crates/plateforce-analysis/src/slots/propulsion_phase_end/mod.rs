//! Where propulsion ends, which one school puts before takeoff and another puts at it.
//!
//! Peak velocity necessarily precedes takeoff, because velocity peaks where net force crosses
//! zero while takeoff is declared later, so propulsion duration is systematically shorter and
//! mean propulsion force systematically higher under `peak_com_velocity` than under `takeoff`.
//! Both appear in the literature and neither acknowledges the other.
//!
//! Which of them ran decides whether a rule splitting this phase at the falling crossing of
//! system weight divides anything: that crossing is the instant `peak_com_velocity` places, so
//! under it the split lands on the phase's own end.

pub mod peak_com_velocity;
pub mod takeoff;

/// The construct id, as `registry/constructs.toml` declares it.
pub const CONSTRUCT: &str = "propulsion_phase_end";

/// The sample a propulsion-end rule placed, under the name later rules read it by.
pub const PLACED: &str = "propulsion_phase_end";

pub const KEY: &str = "propulsion_phase_end_seconds";

/// The boundary a rule needs, or nothing when no rule placed one.
pub fn placed(context: &crate::derived::DerivedContext) -> Option<usize> {
    context.sample(PLACED)
}
