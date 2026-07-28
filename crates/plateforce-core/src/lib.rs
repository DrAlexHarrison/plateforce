//! Force-plate kinetics.
//!
//! Every quantity is computed in exactly one place. The premise of this project is that
//! independent implementations of the same named method disagree, so a second
//! implementation of anything in here would be indefensible.
//!
//! Nothing in this crate decides a method. A caller passes a bound method from the
//! registry and gets a result carrying what produced it.

pub mod onset;
pub mod phases;
pub mod read;
pub mod signal;
pub mod smoothing;
pub mod statistics;
pub mod takeoff;
pub mod trial;

pub use read::{read_delimited_column, read_trial_from_path, ColumnReadReport, ReadError};
pub use signal::{Sentinel, Trial, TrialError};
pub use statistics::{DispersionEstimator, VarianceAccumulation};
pub use trial::{
    flight_time_seconds, jump_height_from_flight_time, jump_height_from_takeoff_velocity,
    reactive_strength_index_modified, takeoff_velocity_meters_per_second, time_to_takeoff_seconds,
    CentralTendency, Landmarks, WeighingEpoch,
};

/// Standard gravity. Declared once because the registry records real instances of
/// implementations disagreeing on it.
pub const STANDARD_GRAVITY_METERS_PER_SECOND_SQUARED: f64 = 9.80665;

/// Reported alongside every computed quantity so a number never travels without the
/// choices that produced it. The absence of anything like this across the seven open
/// force-plate tools is what the registry exists to fix.
#[derive(Debug, Clone, PartialEq)]
pub struct Provenance {
    pub method_id: String,
    pub bound_parameters: Vec<(String, f64)>,
    /// The revision a caller pinned, and None when they pinned none.
    pub registry_version: Option<String>,
    /// Digest of the registry files that were read, measured rather than declared, so it
    /// identifies them without resting on a caller's word. None when no registry was read.
    pub registry_digest: Option<String>,
    /// False when the acquisition block could not be filled, in which case this result
    /// must never be declared to match another lab's.
    pub acquisition_complete: bool,
}

/// A value and the choices that produced it. Nothing user-facing returns a bare f64.
#[derive(Debug, Clone, PartialEq)]
pub struct Measured {
    pub value: f64,
    pub unit: &'static str,
    pub provenance: Provenance,
}

/// What a computation dropped and under which rule. Silent exclusion is the failure
/// mode the registry documents, and silent inclusion of a sentinel is the same defect
/// wearing the opposite costume.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Exclusions {
    pub dropped_samples: usize,
    pub reason: Option<String>,
    pub sentinel_convention: Option<String>,
}
