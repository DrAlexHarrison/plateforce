//! The interval from the placed onset to the placed takeoff.
//!
//! Bounded by two threshold crossings and nothing else, which makes it the least reproducible
//! interval on the trace: both ends move when either rule moves, and every rate and mean taken
//! over it moves with them.

pub mod onset_to_takeoff;

/// The construct id, as `registry/constructs.toml` declares it.
pub const CONSTRUCT: &str = "time_to_takeoff";

pub const KEY: &str = "time_to_takeoff_seconds";
