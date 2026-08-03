//! Where propulsion begins, under the jump literature's partition by velocity sign.
//!
//! The velocity zero crossing is unanimous across every school found and its three published
//! formulations, minimum displacement, zero velocity and zero cumulative net impulse, are
//! analytically identical, which is why they never generated a debate. The threshold form is
//! an acknowledged pragmatic offset against jitter, and the peak-force form is a legacy rule
//! whose instant has no mechanical reason to be the transition.

pub mod peak_grf;
pub mod velocity_threshold;
pub mod zero_velocity;

/// The construct id, as `registry/constructs.toml` declares it.
pub const CONSTRUCT: &str = "propulsion_phase_start";

/// The sample a propulsion-start rule placed, under the name later rules read it by.
pub const PLACED: &str = "propulsion_phase_start";

/// The key every rule here reports under, so they are three answers to one question.
pub const KEY: &str = "propulsion_phase_start_seconds";

/// The boundary a rule needs, or nothing when no rule placed one.
pub fn placed(context: &crate::derived::DerivedContext) -> Option<usize> {
    context.sample(PLACED)
}
