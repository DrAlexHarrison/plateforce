//! One directory per construct a rule can fill, and one module per bound rule inside it.
//!
//! A rule is added by adding a file and one line to its construct's `mod.rs`, so two agents
//! working on different rules in one construct never share a file.

pub mod analysis_window;
pub mod movement_onset;
pub mod peak_force;
pub mod system_weight;
pub mod takeoff;
