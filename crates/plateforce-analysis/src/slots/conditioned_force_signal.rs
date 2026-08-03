//! The signal the landmark rules read, and the record of what produced it.

pub mod none;

/// As `registry/constructs.toml` declares it.
pub const CONSTRUCT: &str = "conditioned_force_signal";

/// The rule this phase runs when a request names none.
///
/// A default that is not on the record is an absence, and an absence reads the same as a
/// filter nobody wrote down. This one is recorded like any other choice, marked as the
/// software's rather than the caller's.
pub const DECLARED_DEFAULT: &str = none::ID;
