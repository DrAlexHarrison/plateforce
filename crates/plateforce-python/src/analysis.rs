//! Running bound methods over a trial.
//!
//! Nothing here computes a quantity. Each step calls the one implementation in
//! `plateforce-core` and wraps what comes back in the choices that produced it.

use plateforce_core::onset::{
    onset_noise_relative, BandSides, CrossingSearch, CrossingSelection, DegenerateBandPolicy,
};
use plateforce_core::statistics::{samples_for_duration, DurationRounding};
use plateforce_core::takeoff::{takeoff_first_sustained_run, ResidualComparison};
use plateforce_core::{
    jump_height_from_flight_time as core_jump_height_from_flight_time,
    jump_height_from_takeoff_velocity, reactive_strength_index_modified,
    takeoff_velocity_meters_per_second, time_to_takeoff_seconds, CentralTendency,
    DispersionEstimator, Landmarks, Measured as CoreMeasured, Provenance as CoreProvenance,
    WeighingEpoch, STANDARD_GRAVITY_METERS_PER_SECOND_SQUARED,
};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

use crate::errors::{map_trial_error, parameter_error, MethodNotImplementedError};
use crate::registry::{BoundMethod, UNVERSIONED};
use crate::result::{Exclusions, Measured, ProvenanceChain};
use crate::trial::Trial;

/// Registry entries this build can run. The registry describes the literature, which is
/// larger than what is implemented, and selecting an entry that is only described has to
/// fail rather than quietly resolve to something near it.
pub const IMPLEMENTED_METHOD_IDS: &[&str] = &[
    "bwepoch.fixed_window",
    "onset.threshold.noise_relative",
    "takeoff.threshold.absolute_force",
];

/// Steps the software performs that no registry entry describes, reported on every result
/// rather than left to be discovered. Gravity is here because the core takes it as an
/// argument, tools disagree on 9.81 against 9.80665, and no entry covers the choice.
const TAKEOFF_VELOCITY_METHOD_ID: &str = "takeoff_velocity.impulse_momentum";
const JUMP_HEIGHT_FROM_VELOCITY_METHOD_ID: &str = "jump_height.from_takeoff_velocity";
const JUMP_HEIGHT_FROM_FLIGHT_TIME_METHOD_ID: &str = "jump_height.from_flight_time";
const TIME_TO_TAKEOFF_METHOD_ID: &str = "time_to_takeoff.onset_to_takeoff";
const RSI_MODIFIED_METHOD_ID: &str = "rsi_modified.height_over_time_to_takeoff";

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
    choices: Vec<(String, String)>,
    depends_on: Vec<ProvenanceChain>,
) -> Measured {
    Measured::new(
        CoreMeasured {
            value,
            unit,
            provenance,
        },
        choices,
        depends_on,
    )
}

fn choice(name: &str, value: &str) -> (String, String) {
    (name.to_string(), value.to_string())
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

fn expect_method(method: &BoundMethod, wanted: &str, role: &str) -> PyResult<()> {
    if method.method_id() == wanted {
        return Ok(());
    }
    Err(MethodNotImplementedError::new_err(format!(
        "'{}' was passed as the {role} method, and '{}' is the rule available for that step. Available: {:?}",
        method.method_id(),
        wanted,
        IMPLEMENTED_METHOD_IDS
    )))
}

fn parse_choice<T: Copy>(name: &str, given: &str, options: &[(&str, T)]) -> PyResult<T> {
    options
        .iter()
        .find(|(label, _)| *label == given)
        .map(|(_, value)| *value)
        .ok_or_else(|| {
            let offered: Vec<&str> = options.iter().map(|(label, _)| *label).collect();
            PyValueError::new_err(format!("{name} must be one of {offered:?}, got '{given}'"))
        })
}

/// The results of one countermovement jump, each carrying the chain of choices behind it.
#[pyclass(frozen, module = "plateforce", name = "CountermovementJump")]
pub struct CountermovementJump {
    system_weight_newtons: Measured,
    system_mass_kilograms: Measured,
    weighing_epoch_tied_window_count: usize,
    onset_index: usize,
    onset_time_seconds: Measured,
    takeoff_index: usize,
    takeoff_time_seconds: Measured,
    time_to_takeoff_seconds: Measured,
    takeoff_velocity_meters_per_second: Measured,
    jump_height_takeoff_frame_meters: Measured,
    reactive_strength_index_modified: Option<Measured>,
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

    /// Windows the weighing rule could not choose between. One for a fixed window.
    /// Anything above one means the selection is an artefact of the arithmetic.
    #[getter]
    fn weighing_epoch_tied_window_count(&self) -> usize {
        self.weighing_epoch_tied_window_count
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

    /// The metric on which open implementations disagree most: two of them agree at
    /// r = 0.696 on this while agreeing at r = 0.961 on jump height.
    #[getter]
    fn time_to_takeoff_seconds(&self) -> Measured {
        self.time_to_takeoff_seconds.clone()
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

    /// None when time to takeoff is not positive, which is the only case the core
    /// declines to divide.
    #[getter]
    fn reactive_strength_index_modified(&self) -> Option<Measured> {
        self.reactive_strength_index_modified.clone()
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
            "CountermovementJump(jump_height_takeoff_frame_meters={:.4}, time_to_takeoff_seconds={:.4}, unregistered_methods={})",
            self.jump_height_takeoff_frame_meters.value_for_display(),
            self.time_to_takeoff_seconds.value_for_display(),
            self.unregistered_methods.len()
        )
    }
}

/// Analyse one countermovement jump with the methods named.
///
/// The three method arguments are bound registry entries and appear in the provenance of
/// every result. The keyword arguments after them are choices the registry states as
/// enumerations rather than numbers, so they land in `provenance.enumerated_choices`
/// rather than in `bound_parameters`, which holds only `(name, float)` pairs.
///
/// `onset_search_bound_seconds` defaults to the whole trace. It bounds how far the onset
/// rule looks before giving up and never moves an onset that was found.
#[pyfunction]
#[pyo3(signature = (
    trial,
    weighing_epoch,
    onset,
    takeoff,
    gravity_meters_per_second_squared = STANDARD_GRAVITY_METERS_PER_SECOND_SQUARED,
    dispersion_estimator = "sample",
    central_tendency = "mean",
    onset_band_sides = "below_only",
    onset_crossing_selection = "first",
    onset_persistence_samples = 1,
    onset_search_bound_seconds = None,
    onset_degenerate_band = "refuse",
    onset_degenerate_band_fraction = None,
    takeoff_residual_comparison = "signed_value",
    duration_rounding = "nearest",
))]
#[allow(clippy::too_many_arguments)]
pub fn analyse_countermovement_jump(
    python: Python<'_>,
    trial: &Trial,
    weighing_epoch: &BoundMethod,
    onset: &BoundMethod,
    takeoff: &BoundMethod,
    gravity_meters_per_second_squared: f64,
    dispersion_estimator: &str,
    central_tendency: &str,
    onset_band_sides: &str,
    onset_crossing_selection: &str,
    onset_persistence_samples: usize,
    onset_search_bound_seconds: Option<f64>,
    onset_degenerate_band: &str,
    onset_degenerate_band_fraction: Option<f64>,
    takeoff_residual_comparison: &str,
    duration_rounding: &str,
) -> PyResult<CountermovementJump> {
    expect_method(weighing_epoch, "bwepoch.fixed_window", "weighing epoch")?;
    expect_method(onset, "onset.threshold.noise_relative", "onset")?;
    expect_method(takeoff, "takeoff.threshold.absolute_force", "takeoff")?;

    let dispersion = parse_choice(
        "dispersion_estimator",
        dispersion_estimator,
        &[
            ("population", DispersionEstimator::Population),
            ("sample", DispersionEstimator::Sample),
        ],
    )?;
    let centre = parse_choice(
        "central_tendency",
        central_tendency,
        &[
            ("mean", CentralTendency::Mean),
            ("median", CentralTendency::Median),
        ],
    )?;
    let band_sides = parse_choice(
        "onset_band_sides",
        onset_band_sides,
        &[
            ("below_only", BandSides::BelowOnly),
            ("both_sides", BandSides::BothSides),
        ],
    )?;
    let selection = parse_choice(
        "onset_crossing_selection",
        onset_crossing_selection,
        &[
            ("first", CrossingSelection::First),
            ("last", CrossingSelection::Last),
        ],
    )?;
    let comparison = parse_choice(
        "takeoff_residual_comparison",
        takeoff_residual_comparison,
        &[
            ("signed_value", ResidualComparison::SignedValue),
            ("magnitude", ResidualComparison::Magnitude),
        ],
    )?;
    let rounding = parse_choice(
        "duration_rounding",
        duration_rounding,
        &[
            ("truncate", DurationRounding::Truncate),
            ("nearest", DurationRounding::Nearest),
            ("ceiling", DurationRounding::Ceiling),
        ],
    )?;

    // Carries a number, so it is read here rather than through the table above.
    let degenerate = match onset_degenerate_band {
        "refuse" => DegenerateBandPolicy::Refuse,
        "use_as_stated" => DegenerateBandPolicy::UseAsStated,
        "fraction_of_reference" => match onset_degenerate_band_fraction {
            Some(fraction) if fraction.is_finite() => {
                DegenerateBandPolicy::FractionOfReference(fraction)
            }
            _ => {
                return Err(PyValueError::new_err(
                    "onset_degenerate_band='fraction_of_reference' needs onset_degenerate_band_fraction set to a finite number",
                ))
            }
        },
        other => {
            return Err(PyValueError::new_err(format!(
                "onset_degenerate_band must be one of [\"refuse\", \"use_as_stated\", \"fraction_of_reference\"], got '{other}'"
            )))
        }
    };

    let registry_version = onset.registry_version();
    let acquisition_complete = trial.acquisition_complete();
    let sample_rate_hz = trial.inner.sample_rate_hz();

    let epoch_duration_seconds = required_parameter(weighing_epoch, "duration")?;
    let epoch =
        WeighingEpoch::fixed_window(&trial.inner, epoch_duration_seconds, centre, dispersion)
            .map_err(|error| map_trial_error(python, error))?;

    let epoch_choices = vec![
        choice("central_tendency", central_tendency),
        choice("dispersion_estimator", dispersion_estimator),
    ];
    let epoch_provenance = provenance(
        weighing_epoch.method_id(),
        weighing_epoch.bound_parameters.clone(),
        registry_version,
        acquisition_complete,
    );
    let epoch_chain =
        ProvenanceChain::leaf(epoch_provenance.clone()).choosing(epoch_choices.clone());

    let search_end_seconds =
        onset_search_bound_seconds.unwrap_or_else(|| trial.inner.duration_seconds());
    let search = CrossingSearch {
        start_index: epoch.end_index,
        end_index: samples_for_duration(search_end_seconds, sample_rate_hz, rounding)
            .min(trial.inner.len()),
        persistence_samples: onset_persistence_samples,
        selection,
    };
    let onset_index = onset_noise_relative(
        trial.inner.force(),
        epoch.system_weight_newtons,
        epoch.standard_deviation_newtons,
        required_parameter(onset, "k")?,
        band_sides,
        // Refuses by default. A collapsed band means the window the rule assumed was
        // quiet was not, and a silent substitution would hide that.
        degenerate,
        &search,
        sample_rate_hz,
    )
    .map_err(|error| map_trial_error(python, error))?;

    // The registry states this parameter in milliseconds and the core takes seconds. The
    // conversion is the only arithmetic in this file and it is a unit change, not a rule.
    let minimum_flight_samples = match takeoff.value_of("persistence_ms") {
        Some(milliseconds) => {
            samples_for_duration(milliseconds / 1000.0, sample_rate_hz, rounding).max(1)
        }
        None => 1,
    };
    let takeoff_index = takeoff_first_sustained_run(
        trial.inner.force(),
        required_parameter(takeoff, "threshold_n")?,
        minimum_flight_samples,
        comparison,
        onset_index,
        sample_rate_hz,
    )
    .map_err(|error| map_trial_error(python, error))?;

    let onset_choices = vec![
        choice("band_sides", onset_band_sides),
        choice("crossing_selection", onset_crossing_selection),
        choice("degenerate_band", onset_degenerate_band),
        choice("dispersion_estimator", dispersion_estimator),
    ];
    let onset_provenance = provenance(
        onset.method_id(),
        onset.bound_parameters.clone(),
        registry_version,
        acquisition_complete,
    );
    let onset_chain = ProvenanceChain::with_inputs(onset_provenance.clone(), vec![epoch_chain.clone()])
        .choosing(onset_choices.clone());

    let takeoff_choices = vec![choice("residual_comparison", takeoff_residual_comparison)];
    let takeoff_provenance = provenance(
        takeoff.method_id(),
        takeoff.bound_parameters.clone(),
        registry_version,
        acquisition_complete,
    );
    let takeoff_chain =
        ProvenanceChain::with_inputs(takeoff_provenance.clone(), vec![epoch_chain.clone()])
            .choosing(takeoff_choices.clone());

    // No registry rule places landing here, and nothing below reads it.
    let landmarks = Landmarks {
        onset_index,
        takeoff_index,
        touchdown_index: takeoff_index,
    };

    let gravity_parameter = vec![(
        "gravity_meters_per_second_squared".to_string(),
        gravity_meters_per_second_squared,
    )];

    let time_to_takeoff = time_to_takeoff_seconds(&landmarks, trial.inner.sample_interval_seconds());
    let time_to_takeoff_measured = measured(
        time_to_takeoff,
        "seconds",
        provenance(
            TIME_TO_TAKEOFF_METHOD_ID,
            Vec::new(),
            registry_version,
            acquisition_complete,
        ),
        Vec::new(),
        vec![onset_chain.clone(), takeoff_chain.clone()],
    );

    let velocity = takeoff_velocity_meters_per_second(
        &trial.inner,
        &epoch,
        &landmarks,
        gravity_meters_per_second_squared,
    );
    let velocity_measured = measured(
        velocity,
        "meters_per_second",
        provenance(
            TAKEOFF_VELOCITY_METHOD_ID,
            gravity_parameter.clone(),
            registry_version,
            acquisition_complete,
        ),
        Vec::new(),
        vec![epoch_chain.clone(), onset_chain.clone(), takeoff_chain.clone()],
    );

    let jump_height =
        jump_height_from_takeoff_velocity(velocity, gravity_meters_per_second_squared);
    let jump_height_measured = measured(
        jump_height,
        "meters",
        provenance(
            JUMP_HEIGHT_FROM_VELOCITY_METHOD_ID,
            gravity_parameter,
            registry_version,
            acquisition_complete,
        ),
        Vec::new(),
        vec![velocity_measured.chain()],
    );

    let rsi_measured =
        reactive_strength_index_modified(jump_height, time_to_takeoff).map(|value| {
            measured(
                value,
                "meters_per_second",
                provenance(
                    RSI_MODIFIED_METHOD_ID,
                    Vec::new(),
                    registry_version,
                    acquisition_complete,
                ),
                Vec::new(),
                vec![jump_height_measured.chain(), time_to_takeoff_measured.chain()],
            )
        });

    Ok(CountermovementJump {
        system_weight_newtons: measured(
            epoch.system_weight_newtons,
            "newtons",
            epoch_provenance.clone(),
            epoch_choices.clone(),
            Vec::new(),
        ),
        system_mass_kilograms: measured(
            epoch.system_mass_kilograms(gravity_meters_per_second_squared),
            "kilograms",
            epoch_provenance,
            epoch_choices,
            Vec::new(),
        ),
        weighing_epoch_tied_window_count: epoch.tied_window_count,
        onset_index,
        onset_time_seconds: measured(
            trial.inner.time_at(onset_index),
            "seconds",
            onset_provenance,
            onset_choices,
            vec![epoch_chain.clone()],
        ),
        takeoff_index,
        takeoff_time_seconds: measured(
            trial.inner.time_at(takeoff_index),
            "seconds",
            takeoff_provenance,
            takeoff_choices,
            vec![epoch_chain],
        ),
        time_to_takeoff_seconds: time_to_takeoff_measured,
        takeoff_velocity_meters_per_second: velocity_measured,
        jump_height_takeoff_frame_meters: jump_height_measured,
        reactive_strength_index_modified: rsi_measured,
        trial_exclusions: trial.exclusions_for_result(),
        unregistered_methods: vec![
            TIME_TO_TAKEOFF_METHOD_ID.to_string(),
            TAKEOFF_VELOCITY_METHOD_ID.to_string(),
            JUMP_HEIGHT_FROM_VELOCITY_METHOD_ID.to_string(),
            RSI_MODIFIED_METHOD_ID.to_string(),
        ],
    })
}

/// Jump height from a flight time, in metres.
///
/// A different construct from the takeoff-frame height an analysis returns, not a
/// different way of computing the same one. Exposed on its own because nothing in the core
/// places landing, so a flight time has to come from elsewhere, such as a contact mat.
#[pyfunction]
#[pyo3(signature = (
    flight_time_seconds,
    gravity_meters_per_second_squared = STANDARD_GRAVITY_METERS_PER_SECOND_SQUARED,
    registry_version = None,
    acquisition_complete = false,
))]
pub fn jump_height_from_flight_time(
    flight_time_seconds: f64,
    gravity_meters_per_second_squared: f64,
    registry_version: Option<String>,
    acquisition_complete: bool,
) -> Measured {
    let version = registry_version.unwrap_or_else(|| UNVERSIONED.to_string());
    measured(
        core_jump_height_from_flight_time(flight_time_seconds, gravity_meters_per_second_squared),
        "meters",
        provenance(
            JUMP_HEIGHT_FROM_FLIGHT_TIME_METHOD_ID,
            vec![
                ("flight_time_seconds".to_string(), flight_time_seconds),
                (
                    "gravity_meters_per_second_squared".to_string(),
                    gravity_meters_per_second_squared,
                ),
            ],
            &version,
            acquisition_complete,
        ),
        Vec::new(),
        Vec::new(),
    )
}
