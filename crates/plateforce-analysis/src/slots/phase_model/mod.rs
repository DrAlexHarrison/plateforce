//! How many phases a countermovement jump has, which is a live disagreement rather than a
//! parameter.
//!
//! `mcmahon2018` names one unweighting phase running from the departure below system weight to
//! the return through it, and does not promote the force minimum to a boundary.
//! `harry2020` splits that stretch at the force minimum into unloading and eccentric yielding,
//! on inverse-dynamics joint power, which is an independent measurement channel rather than a
//! rearrangement of the same force trace.
//!
//! So the two produce different sets of metrics rather than different values for one metric,
//! and each rule here declares its own quantity keys. A reader comparing two results sees the
//! keys change.

pub mod downward_upward;
pub mod squat_jump_distinct;
pub mod time_epochs;
pub mod unloading_yielding_split;
pub mod unweighting_single;

/// The construct id, as `registry/constructs.toml` declares it.
pub const CONSTRUCT: &str = "phase_model";
