//! Running a bound method over a trial.
//!
//! Nothing here computes a quantity. Each step calls the one implementation in
//! `plateforce-core` and wraps what comes back in the choices that produced it.

use plateforce_core::trial::{
    onset_noise_relative, takeoff_absolute_threshold, takeoff_velocity_meters_per_second,
};
use plateforce_core::{
    jump_height_from_flight_time as core_jump_height_from_flight_time,
    jump_height_from_takeoff_velocity, Landmarks, Measured as CoreMeasured,
    Provenance as CoreProvenance, WeighingEpoch,
};
use pyo3::prelude::*;

use crate::errors::{map_trial_error, parameter_error, MethodNotImplementedError};
use crate::registry::{BoundMethod, UNVERSIONED};
use crate::result::{Exclusions, Measured, ProvenanceChain};
use crate::trial::Trial;

/// Registry entries this build can run. The registry describes the literature, which is
/// larger than what is implemented, and selecting an entry that is only described has to
/// fail rather than quietly resolve to something near it.
pub const IMPLEMENTED_METHOD_IDS: &[&str] = &[
    "onset.threshold.noise_relative",
    "takeoff.threshold.absolute",
];

/// Steps the software performs that no registry entry yet describes. Reported on every
/// analysis rather than left to be discovered.
const WEIGHING_EPOCH_METHOD_ID: &str = "weighing_epoch.fixed_window";
const TAKEOFF_VELOCITY_METHOD_ID: &str = "takeoff_velocity.impulse_momentum";
const JUMP_HEIGHT_FROM_VELOCITY_METHOD_ID: &str = "jump_height.from_takeoff_velocity";
const JUMP_HEIGHT_FROM_FLIGHT_TIME_METHOD_ID: &str = "jump_height.from_flight_time";

fn provenance(
    method_id: &str,
    bound_parameters: Vec<(String, f64)>,
    registry_version: &str,
    acquisition_complete: bool,
) -> CoreProvenance {
    CoreProvenance {
        method_id: method_id.to_string(),
        bound_parameters,
        registry_version: registry_version.to_string(),
        acquisition_complete,
    }
}

fn measured(
    value: f64,
    unit: &'static str,
    provenance: CoreProvenance,
    depends_on: Vec<ProvenanceChain>,
) -> Measured {
    Measured::new(
        CoreMeasured {
            value,
            unit,
            provenance,
        },
        depends_on,
    )
}

/// Reads a parameter the implementation needs off the bound method, naming both sides
/// when the registry entry does not carry it.
fn required_parameter(method: &BoundMethod, name: &str) -> PyResult<f64> {
    method.value_of(name).ok_or_else(|| {
        parameter_error(
            method.method_id(),
            name,
            format!(
                "{}: this implementation needs parameter '{}', and the registry entry binds {:?}",
                method.method_id(),
                name,
                method.parameter_names()
            ),
        )
    })
}

fn not_implemented(method: &BoundMethod, expected_construct: &str) -> PyErr {
    MethodNotImplementedError::new_err(format!(
        "'{}' is in the registry but this build has no implementation of it. Implemented {} methods: {:?}",
        method.method_id(),
        expected_construct,
        IMPLEMENTED_METHOD_IDS
    ))
}

/// The results of one countermovement jump, each carrying the chain of choices behind it.
#[pyclass(frozen, module = "plateforce", name = "CountermovementJump")]
pub struct CountermovementJump {
    system_weight_newtons: Measured,
    system_mass_kilograms: Measured,
    onset_index: usize,
    onset_time_seconds: Measured,
    takeoff_index: usize,
    takeoff_time_seconds: Measured,
    takeoff_velocity_meters_per_second: Measured,
    jump_height_takeoff_frame_meters: Measured,
    trial_exclusions: Exclusions,
    unregistered_methods: Vec<String>,
}

#[pymethods]
impl CountermovementJump {
    #[getter]
    fn system_weight_newtons(&self) -> Measured {
        self.system_weight_newtons.clone()
    }

    #[getter]
    fn system_mass_kilograms(&self) -> Measured {
        self.system_mass_kilograms.clone()
    }

    #[getter]
    fn onset_index(&self) -> usize {
        self.onset_index
    }

    #[getter]
    fn onset_time_seconds(&self) -> Measured {
        self.onset_time_seconds.clone()
    }

    #[getter]
    fn takeoff_index(&self) -> usize {
        self.takeoff_index
    }

    #[getter]
    fn takeoff_time_seconds(&self) -> Measured {
        self.takeoff_time_seconds.clone()
    }

    #[getter]
    fn takeoff_velocity_meters_per_second(&self) -> Measured {
        self.takeoff_velocity_meters_per_second.clone()
    }

    /// Jump height in the takeoff frame. Not comparable with a standing-frame height
    /// without a declared correction: the two differ by 26 to 45 percent.
    #[getter]
    fn jump_height_takeoff_frame_meters(&self) -> Measured {
        self.jump_height_takeoff_frame_meters.clone()
    }

    /// Samples on the trial that matched a sentinel convention or were not finite.
    #[getter]
    fn trial_exclusions(&self) -> Exclusions {
        self.trial_exclusions.clone()
    }

    /// Method ids used here that no registry entry describes. Every one of them is a
    /// choice that moved the result and that a reader cannot look up.
    #[getter]
    fn unregistered_methods(&self) -> Vec<String> {
        self.unregistered_methods.clone()
    }

    fn __repr__(&self) -> String {
        format!(
            "CountermovementJump(jump_height_takeoff_frame_meters={:.4}, takeoff_velocity_meters_per_second={:.4}, unregistered_methods={})",
            self.jump_height_takeoff_frame_meters.value_for_display(),
            self.takeoff_velocity_meters_per_second.value_for_display(),
            self.unregistered_methods.len()
        )
    }
}

/// Analyse one countermovement jump with the methods named.
///
/// `weighing_epoch_seconds` is a choice, not a constant: the registry records at least
/// four distinct windows in the literature, and one implementation specifies its window in
/// samples rather than seconds, which silently changes meaning between a 1000 Hz and a
/// 1200 Hz recording. It appears in the provenance of every result below.
///
/// `onset_search_bound_seconds` defaults to the whole trace. It bounds how far the onset
/// rule looks before giving up and never changes an onset that was found.
#[pyfunction]
#[pyo3(signature = (
    trial,
    weighing_epoch_seconds,
    onset,
    takeoff,
    onset_search_bound_seconds = None,
))]
pub fn analyse_countermovement_jump(
    python: Python<'_>,
    trial: &Trial,
    weighing_epoch_seconds: f64,
    onset: &BoundMethod,
    takeoff: &BoundMethod,
    onset_search_bound_seconds: Option<f64>,
) -> PyResult<CountermovementJump> {
    let registry_version = onset.registry_version();
    let acquisition_complete = trial.acquisition_complete();

    let epoch = WeighingEpoch::fixed_window(&trial.inner, weighing_epoch_seconds)
        .map_err(|error| map_trial_error(python, error))?;
    let epoch_provenance = provenance(
        WEIGHING_EPOCH_METHOD_ID,
        vec![("duration_seconds".to_string(), weighing_epoch_seconds)],
        registry_version,
        acquisition_complete,
    );
    let epoch_chain = ProvenanceChain::leaf(epoch_provenance.clone());

    let search_bound = onset_search_bound_seconds.unwrap_or_else(|| trial.inner.duration_seconds());
    let onset_index = match onset.method_id() {
        "onset.threshold.noise_relative" => onset_noise_relative(
            &trial.inner,
            &epoch,
            required_parameter(onset, "k")?,
            required_parameter(onset, "back_offset")?,
            search_bound,
        )
        .map_err(|error| map_trial_error(python, error))?,
        _ => return Err(not_implemented(onset, "onset")),
    };

    let takeoff_index = match takeoff.method_id() {
        "takeoff.threshold.absolute" => takeoff_absolute_threshold(
            &trial.inner,
            required_parameter(takeoff, "threshold_newtons")?,
            required_parameter(takeoff, "minimum_flight_seconds")?,
            onset_index,
        )
        .map_err(|error| map_trial_error(python, error))?,
        _ => return Err(not_implemented(takeoff, "takeoff")),
    };

    let onset_provenance = provenance(
        onset.method_id(),
        onset.bound_parameters.clone(),
        registry_version,
        acquisition_complete,
    );
    let takeoff_provenance = provenance(
        takeoff.method_id(),
        takeoff.bound_parameters.clone(),
        registry_version,
        acquisition_complete,
    );
    let onset_chain = ProvenanceChain::with_inputs(onset_provenance.clone(), vec![epoch_chain.clone()]);
    let takeoff_chain =
        ProvenanceChain::with_inputs(takeoff_provenance.clone(), vec![epoch_chain.clone()]);

    // No registry rule places touchdown and nothing below reads it, so it is not exposed.
    let landmarks = Landmarks {
        onset_index,
        takeoff_index,
        touchdown_index: takeoff_index,
    };

    let velocity = takeoff_velocity_meters_per_second(&trial.inner, &epoch, &landmarks);
    let velocity_measured = measured(
        velocity,
        "meters_per_second",
        provenance(
            TAKEOFF_VELOCITY_METHOD_ID,
            Vec::new(),
            registry_version,
            acquisition_complete,
        ),
        vec![epoch_chain.clone(), onset_chain.clone(), takeoff_chain.clone()],
    );

    let jump_height = jump_height_from_takeoff_velocity(velocity);
    let jump_height_measured = measured(
        jump_height,
        "meters",
        provenance(
            JUMP_HEIGHT_FROM_VELOCITY_METHOD_ID,
            Vec::new(),
            registry_version,
            acquisition_complete,
        ),
        vec![velocity_measured.chain()],
    );

    Ok(CountermovementJump {
        system_weight_newtons: measured(
            epoch.system_weight_newtons,
            "newtons",
            epoch_provenance.clone(),
            Vec::new(),
        ),
        system_mass_kilograms: measured(
            epoch.system_mass_kilograms(),
            "kilograms",
            epoch_provenance,
            Vec::new(),
        ),
        onset_index,
        onset_time_seconds: measured(
            trial.inner.time_at(onset_index),
            "seconds",
            onset_provenance,
            vec![epoch_chain.clone()],
        ),
        takeoff_index,
        takeoff_time_seconds: measured(
            trial.inner.time_at(takeoff_index),
            "seconds",
            takeoff_provenance,
            vec![epoch_chain],
        ),
        takeoff_velocity_meters_per_second: velocity_measured,
        jump_height_takeoff_frame_meters: jump_height_measured,
        trial_exclusions: trial.exclusions_for_result(),
        unregistered_methods: vec![
            WEIGHING_EPOCH_METHOD_ID.to_string(),
            TAKEOFF_VELOCITY_METHOD_ID.to_string(),
            JUMP_HEIGHT_FROM_VELOCITY_METHOD_ID.to_string(),
        ],
    })
}

/// Jump height from a flight time, in metres.
///
/// A different construct from the takeoff-frame height an analysis returns, not a
/// different way of computing the same one. Exposed on its own because nothing in the core
/// places touchdown, so a flight time has to come from elsewhere, such as a contact mat.
#[pyfunction]
#[pyo3(signature = (flight_time_seconds, registry_version = None, acquisition_complete = false))]
pub fn jump_height_from_flight_time(
    flight_time_seconds: f64,
    registry_version: Option<String>,
    acquisition_complete: bool,
) -> Measured {
    let version = registry_version.unwrap_or_else(|| UNVERSIONED.to_string());
    measured(
        core_jump_height_from_flight_time(flight_time_seconds),
        "meters",
        provenance(
            JUMP_HEIGHT_FROM_FLIGHT_TIME_METHOD_ID,
            vec![("flight_time_seconds".to_string(), flight_time_seconds)],
            &version,
            acquisition_complete,
        ),
        Vec::new(),
    )
}
