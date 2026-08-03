//! Where braking begins, and the two instants the field calls by that one name.
//!
//! The two rules here are not two estimates of one instant. The force minimum strictly
//! precedes the net-force crossing, because velocity keeps becoming more negative while force
//! sits below system weight, so every mean taken over a braking phase is taken over a longer
//! and earlier window under the first rule than under the second. Two commercial products
//! report drop-jump braking duration differing by 59 percent on one raw trace with an
//! association of R squared 0.003, which is the size of the gap between these two names.
//!
//! The instant travels under one name so a rule bounded by braking start reads the boundary
//! rather than the rule that placed it, and names that rule in its own chain.

pub mod min_force;
pub mod zero_net_force;

/// The construct id, as `registry/constructs.toml` declares it.
pub const CONSTRUCT: &str = "braking_phase_start";

/// The sample a braking-start rule placed, under the name later rules read it by.
pub const PLACED: &str = "braking_phase_start";

/// The key both rules report under. Holding the key still and letting `computed_by` vary is
/// what makes them two answers to one question rather than two quantities.
pub const KEY: &str = "braking_phase_start_seconds";

/// The boundary a rule needs, or nothing when no rule placed one.
pub fn placed(context: &crate::derived::DerivedContext) -> Option<usize> {
    context.sample(PLACED)
}
