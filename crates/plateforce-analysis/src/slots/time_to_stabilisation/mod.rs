//! How long after landing the plate reads quiet standing again.
//!
//! One rule, and it is a vendor rule. The published families beside it, sequential estimation,
//! an unbounded polynomial fit, range-based, cumulative sum, and the dynamic postural
//! stability index, are the largest unresolved cluster in the landing domain and the registry
//! carries them as one entry with the query that would settle them.

pub mod band_and_dwell;

/// The construct id, as `registry/constructs.toml` declares it.
pub const CONSTRUCT: &str = "time_to_stabilisation";

pub const KEY: &str = "time_to_stabilisation_seconds";
