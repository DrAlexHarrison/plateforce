//! Mechanical work over a declared interval, by the two quadrature routes and the vendor
//! product that is not one.
//!
//! Force through a displacement and power through a time are the same quantity in continuous
//! form, so the registry files those two as a naming disagreement rather than as competing
//! schools, and core integrates once for both. Which of the two an implementation reaches for
//! is a question about its inputs: the power-time route needs velocity, one integration deep,
//! and the force-displacement route needs displacement, which is two, so an error in the
//! weighing epoch propagates linearly through the first and quadratically through the second.
//!
//! The third is the vendor construction, a single force value multiplied by a single
//! displacement value with one peak taken per cycle. It equals the integral only where force
//! is constant through the displacement, which it emphatically is not during a jump. It is
//! registered because a generation of results was produced with it, and it reports under the
//! same key so the gap against the integral is readable rather than filed away.

pub mod integral_force_ds;
pub mod integral_power_dt;
pub mod single_product;

/// The construct id, as `registry/constructs.toml` declares it.
pub const CONSTRUCT: &str = "mechanical_work";

/// The key all three rules report under.
pub const KEY: &str = "mechanical_work_joules";
