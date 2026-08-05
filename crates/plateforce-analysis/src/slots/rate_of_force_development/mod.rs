//! How fast force rises, in the published variants that disagree about what that means.
//!
//! At least three incompatible quantities carry this name and one vendor ships four, so the
//! variant and its window travel with every number here. The registry files two commercial
//! packages differing by 138 percent at the 0 to 50 ms epoch on identical isometric traces
//! and by 0.00 percent on peak force from the same recordings, which is the size of what the
//! choice of rule moves.
//!
//! Every rule here reports the rate under one key and lets `computed_by` say which rule
//! produced it, on the model the three peak-force rules already follow. One key and one
//! number each: a request carries one rule for this construct, so a rule reporting a second
//! quantity of its own would cost a caller who reached for any other rule that quantity, with
//! nothing on the result saying so.
//!
//! What each rule reads for its interval differs on purpose and follows each entry's own
//! stated sensitivity. The two onset-anchored schemes read the placed onset. The steepest
//! chord reads the analysis window and never the onset, because the entry's whole claim for
//! it is that it is onset-independent and a search bounded at onset would not be. The two
//! phase-anchored rules read the propulsion boundaries the caller's phase rules placed.

pub mod at_fraction_of_peak;
pub mod average_to_peak;
pub mod between_force_levels;
pub mod epoch_overlapping;
pub mod epoch_sequential;
pub mod exponential_model;
pub mod mean_force_over_duration;
pub mod peak_sliding_window;
pub mod phase_endpoint_secant;

/// The construct id, as `registry/constructs.toml` declares it.
pub const CONSTRUCT: &str = "rate_of_force_development";

/// The key every rule here reports its rate under. Holding the key still and letting
/// `computed_by` vary is what makes them answers to one question rather than separate
/// quantities.
pub const KEY: &str = "rate_of_force_development_newtons_per_second";
