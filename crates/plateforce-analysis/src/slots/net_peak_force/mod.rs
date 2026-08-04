//! The biggest force with the athlete's own weight taken out of it.
//!
//! Filed apart from `peak_force` because a request carries one rule per construct and this rule
//! reports a key the peak force rules never report. Measured on the synthetic trace and on
//! subject 01 trial 1: `force.peak.gross` and `force.peak.estimator` both report
//! `peak_force_newtons`, `force.peak.net` reports `net_peak_force_newtons` and neither of the
//! others, so from one slot a caller naming net lost the gross number and nothing on the result
//! said so.
//!
//! The gap between the two is one system weight, which is fixed in newtons and movement-dependent
//! as a fraction: over the six committed trials system weight is 0.583 to 0.643 of the net peak.
//! `CONVENTIONS.md` section 2 fixes the pair of names so a reader cannot take one for the other.

pub mod net;

/// The construct id, as `registry/constructs.toml` declares it.
pub const CONSTRUCT: &str = "net_peak_force";

/// The key this construct reports under, held still so a rule added beside it is another answer
/// to one question rather than another quantity.
pub const KEY: &str = "net_peak_force_newtons";
