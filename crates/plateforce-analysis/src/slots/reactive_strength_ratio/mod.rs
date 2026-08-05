//! Time in the air per unit of the time taken to get there, as a pure number.
//!
//! Dimensionless, where the index beside it is a velocity, and the two cannot be converted by
//! any constant because jump height depends on flight time quadratically. Two athletes with
//! identical ratios can rank differently on the index.

pub mod ft_over_ttt_cmj;

/// The construct id, as `registry/constructs.toml` declares it.
pub const CONSTRUCT: &str = "reactive_strength_ratio";

pub const KEY: &str = "reactive_strength_ratio";
