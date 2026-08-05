//! One directory per construct a rule can fill, and one module per bound rule inside it.
//!
//! A rule is added by adding a file and one line to its construct's `mod.rs`.

pub mod analysis_window;
pub mod braking_phase_start;
pub mod conditioned_force_signal;
pub mod epoch_impulse;
pub mod flight_time;
pub mod force_at_epoch;
pub mod jh_standing_frame;
pub mod jh_takeoff_frame;
pub mod jh_undeclared;
pub mod jump_type;
pub mod landing;
pub mod landing_phase_end;
pub mod landing_subdivision;
pub mod lifting_phase_end;
pub mod lifting_phase_start;
pub mod mechanical_object;
pub mod mechanical_power;
pub mod mechanical_work;
pub mod movement_onset;
pub mod net_impulse;
pub mod net_peak_force;
pub mod normalisation_basis;
pub mod peak_force;
pub mod phase_model;
pub mod power_mean;
pub mod power_peak;
pub mod propulsion_phase_end;
pub mod propulsion_phase_start;
pub mod propulsion_subdivision;
pub mod rate_of_force_development;
pub mod rate_of_power_development;
pub mod reactive_strength_index;
pub mod sticking_region;
pub mod system_weight;
pub mod takeoff;
pub mod time_to_takeoff;
pub mod trial_validity;
