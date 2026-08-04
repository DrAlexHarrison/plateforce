//! Jump height per unit of the time taken to produce it.
//!
//! The registry carries two numerators under this construct and they are different numbers
//! from one recording, so which one produced a figure is what its entry id says.

pub mod jh_tov_over_ttt;

/// The construct id, as `registry/constructs.toml` declares it.
pub const CONSTRUCT: &str = "reactive_strength_index";

pub const KEY: &str = "reactive_strength_index_modified";
