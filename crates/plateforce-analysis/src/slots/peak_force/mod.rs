//! The biggest force, and the three published answers to what that means.
//!
//! Every rule here reads the window `analysis_window` placed and none of them chooses one.
//! A peak taken over the whole recording is the landing on a countermovement jump, not the
//! jump, so the window is the larger choice of the two and it is recorded as its own.
//!
//! The three disagree in ways worth stating, because they are what this construct is for.
//! Gross and net differ by exactly one system weight, which is movement-independent in
//! newtons and entirely movement-dependent as a fraction: measured over the six committed
//! trials, system weight is 0.5826 to 0.6431 of the net peak, mean 0.6167. The estimator
//! reads the maximum of a centred average rather than of a single sample, and against gross
//! at the 0.1 s window its published pair names, the gap over those trials runs 27.1 to
//! 60.8 N, mean 44.2 N.

pub mod estimator;
pub mod gross;
pub mod net;

/// The construct id, as `registry/constructs.toml` declares it.
pub const CONSTRUCT: &str = "peak_force";

/// The key all three report under. Holding the key still and letting `computed_by` vary is
/// what makes them three answers to one question rather than three quantities.
pub const KEY: &str = "peak_force_newtons";

/// The one that is a different quantity rather than a different rule. Net is the peak after
/// system weight has been subtracted, and `CONVENTIONS.md` section 2 fixes the pair of names
/// so a reader cannot take one for the other.
pub const NET_KEY: &str = "net_peak_force_newtons";
