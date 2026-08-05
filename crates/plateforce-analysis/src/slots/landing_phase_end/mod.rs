//! Where the landing ends, and the one of the two published answers a force plate can see.
//!
//! `phase.landing_end.zero_com_velocity` reads the reconstructed centre of mass and
//! `phase.landing_end.peak_knee_flexion` reads a joint angle, so the second is gated on an
//! instrument rather than on a movement. The registry says the gating is itself
//! registry-relevant: a force-plate package can implement only one of them and should say so
//! rather than silently substituting. That is the barrier declared beside the entry.
//!
//! Every quantity here reads past takeoff, so its denominator is the trials that hold a
//! landing rather than the trials in the corpus. Five of the six committed fixtures end with
//! the athlete still airborne.

pub mod zero_com_velocity;

/// The construct id, as `registry/constructs.toml` declares it.
pub const CONSTRUCT: &str = "landing_phase_end";

/// The key every rule here reports under. Holding the key still and letting `computed_by`
/// vary is what makes two rules answers to one question rather than two quantities.
pub const KEY: &str = "landing_phase_end_seconds";

/// The sample a rule here placed, under the name later rules read it by.
pub const PLACED: &str = "landing_phase_end";

/// Where the landing ended, or nothing when no rule placed it.
pub fn placed(context: &crate::derived::DerivedContext) -> Option<usize> {
    context.sample(PLACED)
}
