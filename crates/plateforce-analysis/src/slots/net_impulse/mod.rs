//! The force above standing weight, added up over the interval that produced the jump.
//!
//! The construct holds the takeoff velocity with it, because one entry describes both: the
//! velocity is the impulse divided by the mass the impulse accelerated. They are two numbers
//! and they rest on different things, since the impulse is integrated directly while the
//! velocity is read off an integrated series.

pub mod as_performance_determinant;

/// The construct id, as `registry/constructs.toml` declares it.
pub const CONSTRUCT: &str = "net_impulse";

pub const KEY: &str = "net_impulse_newton_seconds";

/// Filed here rather than under the `takeoff_velocity` construct, whose registry entries are
/// the integration choices this number is read under rather than alternative ways to reach it.
pub const VELOCITY_KEY: &str = "takeoff_velocity_meters_per_second";
