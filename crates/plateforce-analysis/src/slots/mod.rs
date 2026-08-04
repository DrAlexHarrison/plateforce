//! One directory per construct a rule can fill, and one module per bound rule inside it.
//!
//! A rule is added by adding a file and one line to its construct's `mod.rs`, so two agents
//! working on different rules in one construct never share a file.

pub mod analysis_window;
pub mod braking_phase_start;
pub mod conditioned_force_signal;
pub mod flight_time;
pub mod jh_standing_frame;
pub mod jh_takeoff_frame;
pub mod jh_undeclared;
pub mod movement_onset;
pub mod net_impulse;
pub mod peak_force;
pub mod phase_model;
pub mod propulsion_phase_end;
pub mod propulsion_phase_start;
pub mod reactive_strength_index;
pub mod system_weight;
pub mod takeoff;
pub mod time_to_takeoff;
