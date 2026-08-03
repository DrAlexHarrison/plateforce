//! Rise of the centre of mass above where it stood at the instant of takeoff.
//!
//! The frame is the whole reason the published methods for jump height disagree as widely as
//! they do. Standing frame minus takeoff frame is 26 to 45 percent, definitional throughout,
//! so a height carrying no frame is not a number that can be compared with another one.
//!
//! Four rules, and the registry files all four here because they estimate the one quantity.
//! Flight time reports under its own key rather than this construct's: the projectile
//! equation is a biased estimator of the takeoff frame, published at +0.021 m against
//! impulse-momentum across nine studies, and the two are drawn beside each other so a reader
//! can see the gap on their own trial.

pub mod flight_time;
pub mod impulse_momentum;
pub mod peak_velocity;
pub mod work_energy;

/// The construct id, as `registry/constructs.toml` declares it.
pub const CONSTRUCT: &str = "jump_height.takeoff_frame";

/// The key three of the four report under. Holding it still and letting `computed_by` vary is
/// what makes them three answers to one question rather than three quantities.
pub const KEY: &str = "jump_height_from_takeoff_meters";

/// The flight-time route's own key. It is the height the projectile equation gives, and the
/// quality signal that checks the two routes against each other reads them by these two
/// names, so the pair has to stay separable.
pub const FLIGHT_TIME_KEY: &str = "jump_height_from_flight_time_meters";
