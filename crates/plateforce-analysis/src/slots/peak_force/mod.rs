//! The biggest force, with the athlete's weight inside the number, and the published answers
//! to what that means.
//!
//! Every rule here reads the window `analysis_window` placed and none of them chooses one.
//! A peak taken over the whole recording is the landing on a countermovement jump, not the
//! jump, so the window is the larger choice of the two and it is recorded as its own.
//!
//! The estimator reads the maximum of a centred average rather than of a single sample, and
//! against gross at the 0.1 s window its published pair names, the gap over the six committed
//! trials runs 27.1 to 60.8 N, mean 44.2 N. That is the size of the disagreement this construct
//! carries. The peak after system weight is taken out is a whole system weight away, 0.583 to
//! 0.643 of the net peak over those same trials, and it is `net_peak_force`.

pub mod estimator;
pub mod gross;

/// The construct id, as `registry/constructs.toml` declares it.
pub const CONSTRUCT: &str = "peak_force";

/// The key every rule here reports under. Holding the key still and letting `computed_by` vary
/// is what makes them answers to one question rather than separate quantities.
pub const KEY: &str = "peak_force_newtons";
