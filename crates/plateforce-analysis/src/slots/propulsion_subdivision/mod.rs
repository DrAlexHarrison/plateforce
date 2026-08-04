//! Where the propulsion phase divides into two named sub-phases, which two published sources
//! answer differently.
//!
//! `by_time` splits at a stated share of the interval's duration, arbitrary and reproducible.
//! `by_force_crossing` splits where force descends through system weight, which is also peak
//! centre of mass velocity and zero centre of mass displacement. Both partition the interval
//! the propulsion rules placed, and they land at different instants on one trace.
//!
//! Filed apart from `phase_model` because a request carries one rule per construct and these
//! rules produce quantities the phase models do not. Measured on subject 01 trial 1: the
//! single-unweighting model reports two keys, `by_time` reports one, and the two sets have an
//! empty intersection, so a caller who can name only one of them loses a quantity rather than
//! a spelling. The models decide whether the force minimum is promoted in the countermovement
//! and say nothing about where the push divides.

pub mod by_force_crossing;
pub mod by_time;

/// The construct id, as `registry/constructs.toml` declares it.
pub const CONSTRUCT: &str = "propulsion_subdivision";
