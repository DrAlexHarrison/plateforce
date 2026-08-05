//! Power averaged across a declared interval.
//!
//! Separate from the peak because the two differ by more than any rule for either of them
//! moves it. It degrades gracefully with sampling rate where the peak does not, and one
//! optimal-load lineage chose it for that reason, which changes the reported optimal load.
//!
//! A mean without its interval is not interpretable, so every rule here reads a phase and the
//! chain behind the number names the rules that placed both of its ends.

pub mod phase_mean;

/// The construct id, as `registry/constructs.toml` declares it.
pub const CONSTRUCT: &str = "mechanical_power.mean";

/// The key every rule here reports under.
pub const KEY: &str = "mean_power_watts";
