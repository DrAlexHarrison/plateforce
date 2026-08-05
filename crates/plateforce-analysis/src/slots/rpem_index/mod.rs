//! Jump height per unit of a push-off duration derived from a distance and a velocity.
//!
//! Its own construct rather than a third rule under `reactive_strength_index`, because the
//! registry's own account of the entry is that it is a new quantity rather than a variant:
//! substituting push-off distance over velocity for ground contact time is what makes it
//! computable on a countermovement jump, where there is no contact time in the drop-jump
//! sense. A caller who picked it in the index's slot would lose the index and get a number
//! three times the size under the same heading.

pub mod push_off_distance_over_velocity;

/// The construct id, as `registry/constructs.toml` declares it.
pub const CONSTRUCT: &str = "reactive_strength_index.rpem";

pub const KEY: &str = "rpem_index_meters_per_second";
