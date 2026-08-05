//! What a force, rate, power, work or impulse is expressed relative to.
//!
//! Relative power in the weightlifting literature and in strength and conditioning are
//! different quantities with the same name: one normalises to barbell mass and the other to
//! body mass, and across a competition field the athlete-to-bar ratio varies by a factor of
//! two. A per-kilogram label that does not say which kilograms breaks cross-study comparison
//! without ever looking wrong.

pub mod denominator;

/// The construct id, as `registry/constructs.toml` declares it.
pub const CONSTRUCT: &str = "normalisation_basis";

/// The key the declaration reports the divisor under.
pub const KEY: &str = "normalisation_denominator_kilograms";
