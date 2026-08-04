//! Rise of the centre of mass above where it stood in quiet upright standing.
//!
//! Larger than the takeoff frame by the heel rise, being ankle plantarflexion, foot length and
//! whatever hip and knee extension is left at takeoff. That difference is 26 to 45 percent on
//! average and up to about 15 cm absolute, and it is definitional rather than error: both
//! frames are physically valid and neither is a better estimate of the other.

pub mod double_integral;
pub mod heel_rise_constant;
pub mod tov_plus_rise;

/// The construct id, as `registry/constructs.toml` declares it.
pub const CONSTRUCT: &str = "jump_height.standing_frame";

/// The key all three rules report under, named for the frame rather than for any one route, so
/// they read as three answers to one question.
pub const KEY: &str = "jump_height_from_standing_meters";
