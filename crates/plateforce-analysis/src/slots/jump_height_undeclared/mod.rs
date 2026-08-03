//! Jump height under a rule that does not say which displacement it measures.
//!
//! A height with no frame cannot be compared with another height, because the two frames
//! differ by more than any training effect moves either of them. The rules filed here either
//! settle that, by declaring the frame and computing nothing, or reproduce it, by computing a
//! number whose published rule never says which rise it is.

pub mod flight_phase_displacement;
pub mod frame;
pub mod minimum_of_two_routes;

/// The construct id, as `registry/constructs.toml` declares it.
pub const CONSTRUCT: &str = "jump_height.undeclared";

/// The key the computing rules report under. Separate from either frame's key, because a
/// number filed under one of those would be claiming the frame its own rule declines to state.
pub const KEY: &str = "jump_height_undeclared_frame_meters";
