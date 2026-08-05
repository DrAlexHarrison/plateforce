//! Where the bar slows down during a maximal lift, and the one of two definitions that is
//! computable from a recording at all.
//!
//! Two definitions and one word, and the authors wrote a paper about the difference.
//! `sticking.region.velocity_minimum` is an interval read off ascent velocity.
//! `sticking.point.failure_location.kompf2016` is where a repetition failed, which cannot be
//! read off a successful one, so its barrier is declared beside it in the registry.
//!
//! The detector returns a not-detected state rather than a number, because lifters who
//! exhibit no sticking point at all have been reported and a rule that always answers would
//! turn one of them into a region.

pub mod velocity_minimum;

/// The construct id, as `registry/constructs.toml` declares it.
pub const CONSTRUCT: &str = "sticking_region";

/// The two ends of the region, reported under two keys because a region has two ends and a
/// reader comparing lifts needs both.
pub const START_KEY: &str = "sticking_region_start_seconds";
pub const END_KEY: &str = "sticking_region_end_seconds";

/// The samples a rule here placed, under the names later rules read them by.
pub const START_PLACED: &str = "sticking_region.start";
pub const END_PLACED: &str = "sticking_region.end";
