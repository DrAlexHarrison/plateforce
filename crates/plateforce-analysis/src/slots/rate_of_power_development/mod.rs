//! How fast power rises, under two anchorings that disagree about where the line is drawn.
//!
//! Materially more reliable than rate of force development on the same trials, ICC 0.95 with
//! CV 7.15 percent against ICC 0.57 with CV 76.45 percent, and the source that measured that
//! recommends it as the replacement. The mechanism is not stated anywhere, and the plausible
//! one is that power multiplies a filtered-by-integration velocity with force, so it is
//! smoother than a raw first derivative.
//!
//! Both rules report the rate under one key and let `computed_by` say which produced it.
//!
//! Neither rule chooses what power is. `power.instantaneous.force_x_velocity` owns that and
//! has a construct of its own, so the series and the two choices behind it are built in
//! `slots::mechanical_power` and every rule across four constructs that reads a power series
//! reads the same one.

pub mod peak_to_peak_anchored;
pub mod phase_anchored;

use crate::derived::DerivedContext;
use crate::slots::mechanical_power;

/// The construct id, as `registry/constructs.toml` declares it.
pub const CONSTRUCT: &str = "rate_of_power_development";

/// The key both rules report under, so they are two answers to one question.
pub const KEY: &str = "rate_of_power_development_watts_per_second";

/// The names both rules cannot run without, as the sweep reads them.
pub const REQUIRED_OPTIONS: &[(&str, &str)] = mechanical_power::REQUIRED_OPTIONS_WITHOUT_PHASE;

/// The power series both rules read, built where every other reader of one builds it.
pub(crate) use mechanical_power::power_series;

/// The entries a rate of power development rests on beyond the rules that placed its
/// landmarks: what power is, and the four integration choices the velocity was read under.
pub(crate) fn record_entries_behind(context: &DerivedContext, onset_index: usize) {
    mechanical_power::record_entries_behind(context, KEY, onset_index);
}
