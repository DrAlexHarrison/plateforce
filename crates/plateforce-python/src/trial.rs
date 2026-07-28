//! The trace, the acquisition block that decides whether it can be compared, and the
//! sentinel reporting that has to happen before anything reads it.

use plateforce_core::signal::{partition_sentinels, Sentinel as CoreSentinel};
use plateforce_core::{Exclusions as CoreExclusions, Trial as CoreTrial};
use pyo3::buffer::PyBuffer;
use pyo3::prelude::*;

use crate::errors::{map_trial_error, TrialError};
use crate::result::Exclusions;

/// NaN never compares equal to itself, so this convention matches no reading and the
/// partition reports only the samples that are not finite.
const NON_FINITE_ONLY: CoreSentinel = CoreSentinel::Value(f64::NAN);

/// The five acquisition facts that are not already on the trial.
///
/// `sample_rate_hz` is the sixth member of the fingerprint's acquisition block and is
/// carried by the `Trial`, so it is not repeated here. A block that cannot be filled
/// fingerprints as incomplete rather than as matching: the most consequential setting in
/// one open tool is a contact debounce living in firmware, and no reanalysis recovers
/// from not knowing it.
#[pyclass(frozen, from_py_object, module = "plateforce", name = "Acquisition")]
#[derive(Clone, Default)]
pub struct Acquisition {
    filter_at_capture: Option<String>,
    tare_state: Option<String>,
    plate_natural_frequency_hz: Option<f64>,
    floor_surface: Option<String>,
    firmware_version: Option<String>,
}

#[pymethods]
impl Acquisition {
    #[new]
    #[pyo3(signature = (
        filter_at_capture = None,
        tare_state = None,
        plate_natural_frequency_hz = None,
        floor_surface = None,
        firmware_version = None,
    ))]
    fn new(
        filter_at_capture: Option<String>,
        tare_state: Option<String>,
        plate_natural_frequency_hz: Option<f64>,
        floor_surface: Option<String>,
        firmware_version: Option<String>,
    ) -> Self {
        Self {
            filter_at_capture,
            tare_state,
            plate_natural_frequency_hz,
            floor_surface,
            firmware_version,
        }
    }

    #[getter]
    fn filter_at_capture(&self) -> Option<&str> {
        self.filter_at_capture.as_deref()
    }

    #[getter]
    fn tare_state(&self) -> Option<&str> {
        self.tare_state.as_deref()
    }

    #[getter]
    fn plate_natural_frequency_hz(&self) -> Option<f64> {
        self.plate_natural_frequency_hz
    }

    #[getter]
    fn floor_surface(&self) -> Option<&str> {
        self.floor_surface.as_deref()
    }

    #[getter]
    fn firmware_version(&self) -> Option<&str> {
        self.firmware_version.as_deref()
    }

    /// True only when every member is present. Anything less and results from this trial
    /// carry `acquisition_complete = False`.
    #[getter]
    fn is_complete(&self) -> bool {
        self.filter_at_capture.is_some()
            && self.tare_state.is_some()
            && self.plate_natural_frequency_hz.is_some()
            && self.floor_surface.is_some()
            && self.firmware_version.is_some()
    }

    /// Names of the members still missing, so a user can see what to go and find.
    #[getter]
    fn missing(&self) -> Vec<&'static str> {
        let mut missing = Vec::new();
        if self.filter_at_capture.is_none() {
            missing.push("filter_at_capture");
        }
        if self.tare_state.is_none() {
            missing.push("tare_state");
        }
        if self.plate_natural_frequency_hz.is_none() {
            missing.push("plate_natural_frequency_hz");
        }
        if self.floor_surface.is_none() {
            missing.push("floor_surface");
        }
        if self.firmware_version.is_none() {
            missing.push("firmware_version");
        }
        missing
    }

    fn __repr__(&self) -> String {
        format!(
            "Acquisition(is_complete={}, missing={:?})",
            if self.is_complete() { "True" } else { "False" },
            self.missing()
        )
    }
}

/// A value a vendor export writes to mean "no measurement".
#[pyclass(frozen, from_py_object, module = "plateforce", name = "Sentinel")]
#[derive(Clone)]
pub struct Sentinel {
    pub(crate) inner: CoreSentinel,
}

#[pymethods]
impl Sentinel {
    /// The convention behind the measured case: a commercial export writes 0.00 to
    /// height, time and reactive strength index together.
    #[staticmethod]
    fn zero() -> Self {
        Self {
            inner: CoreSentinel::Zero,
        }
    }

    #[staticmethod]
    fn negative_one() -> Self {
        Self {
            inner: CoreSentinel::NegativeOne,
        }
    }

    #[staticmethod]
    fn value(missing_value: f64) -> Self {
        Self {
            inner: CoreSentinel::Value(missing_value),
        }
    }

    #[getter]
    fn name(&self) -> String {
        sentinel_name(&self.inner)
    }

    fn __repr__(&self) -> String {
        format!("Sentinel({})", sentinel_name(&self.inner))
    }
}

fn sentinel_name(sentinel: &CoreSentinel) -> String {
    match sentinel {
        CoreSentinel::Zero => "zero".to_string(),
        CoreSentinel::NegativeOne => "negative_one".to_string(),
        CoreSentinel::Value(value) => format!("value({value})"),
    }
}

/// Real measurements separated from sentinels, with what was dropped reported.
#[pyclass(
    frozen,
    skip_from_py_object,
    module = "plateforce",
    name = "SentinelPartition"
)]
pub struct SentinelPartition {
    kept: Vec<f64>,
    dropped_indices: Vec<usize>,
    exclusions: Exclusions,
}

#[pymethods]
impl SentinelPartition {
    #[getter]
    fn kept(&self) -> Vec<f64> {
        self.kept.clone()
    }

    #[getter]
    fn dropped_indices(&self) -> Vec<usize> {
        self.dropped_indices.clone()
    }

    #[getter]
    fn exclusions(&self) -> Exclusions {
        self.exclusions.clone()
    }

    fn __repr__(&self) -> String {
        format!(
            "SentinelPartition(kept={}, dropped={})",
            self.kept.len(),
            self.dropped_indices.len()
        )
    }
}

/// Split a column of results into measurements and sentinels.
///
/// For a table of per-trial results, not for a force trace: removing samples from a trace
/// would close the gap and shift every later timestamp. A trace declares its sentinel
/// convention through `Trial(sentinel=...)`, which reports without removing.
#[pyfunction]
#[pyo3(signature = (values, sentinel))]
pub fn partition_sentinel_values(
    python: Python<'_>,
    values: &Bound<'_, PyAny>,
    sentinel: &Sentinel,
) -> PyResult<SentinelPartition> {
    let values = read_float64_values(python, values, "values")?;
    let (kept, dropped_indices) = partition_sentinels(&values, sentinel.inner);
    let dropped_samples = dropped_indices.len();
    Ok(SentinelPartition {
        kept,
        dropped_indices,
        exclusions: Exclusions {
            inner: CoreExclusions {
                dropped_samples,
                reason: Some(format!(
                    "matched the {} sentinel convention, or was not finite",
                    sentinel_name(&sentinel.inner)
                )),
                sentinel_convention: Some(sentinel_name(&sentinel.inner)),
            },
        },
    })
}

/// A single trial: vertical ground reaction force in newtons against a sample rate.
#[pyclass(frozen, skip_from_py_object, module = "plateforce", name = "Trial")]
pub struct Trial {
    pub(crate) inner: CoreTrial,
    acquisition: Option<Acquisition>,
    exclusions: Exclusions,
}

impl Trial {
    pub(crate) fn exclusions_for_result(&self) -> Exclusions {
        self.exclusions.clone()
    }
}

#[pymethods]
impl Trial {
    /// `force_newtons` accepts any float64 buffer, which covers a numpy array, an
    /// `array.array('d')` and a memoryview, and reads it without a Python-level loop. A
    /// list or tuple is accepted too and converts element by element.
    ///
    /// Declaring `sentinel` reports how many samples match that convention. It never
    /// removes them: closing a gap in a trace would shift every timestamp after it.
    #[new]
    #[pyo3(signature = (force_newtons, sample_rate_hz, acquisition = None, sentinel = None))]
    fn new(
        python: Python<'_>,
        force_newtons: &Bound<'_, PyAny>,
        sample_rate_hz: f64,
        acquisition: Option<Acquisition>,
        sentinel: Option<Sentinel>,
    ) -> PyResult<Self> {
        let values = read_float64_values(python, force_newtons, "force_newtons")?;

        let convention = sentinel
            .as_ref()
            .map(|declared| declared.inner)
            .unwrap_or(NON_FINITE_ONLY);
        let (_, dropped_indices) = partition_sentinels(&values, convention);

        let inner =
            CoreTrial::new(values, sample_rate_hz).map_err(|e| map_trial_error(python, e))?;

        let reason = if dropped_indices.is_empty() {
            None
        } else {
            Some(format!(
                "{} sample(s) reported and kept in place: removing them would shift the time base",
                dropped_indices.len()
            ))
        };
        Ok(Self {
            inner,
            acquisition,
            exclusions: Exclusions {
                inner: CoreExclusions {
                    dropped_samples: dropped_indices.len(),
                    reason,
                    sentinel_convention: sentinel.as_ref().map(|s| sentinel_name(&s.inner)),
                },
            },
        })
    }

    #[getter]
    fn sample_rate_hz(&self) -> f64 {
        self.inner.sample_rate_hz()
    }

    #[getter]
    fn sample_interval_seconds(&self) -> f64 {
        self.inner.sample_interval_seconds()
    }

    #[getter]
    fn duration_seconds(&self) -> f64 {
        self.inner.duration_seconds()
    }

    #[getter]
    fn sample_count(&self) -> usize {
        self.inner.len()
    }

    #[getter]
    fn acquisition(&self) -> Option<Acquisition> {
        self.acquisition.clone()
    }

    /// True only when a complete acquisition block was supplied.
    #[getter]
    pub(crate) fn acquisition_complete(&self) -> bool {
        self.acquisition
            .as_ref()
            .map(|block| block.is_complete())
            .unwrap_or(false)
    }

    /// Samples that match the declared sentinel convention or are not finite. They are
    /// counted, reported and left in place.
    #[getter]
    fn exclusions(&self) -> Exclusions {
        self.exclusions.clone()
    }

    /// The trace as a list of floats. This is a copy; the trial owns its own samples.
    #[getter]
    fn force_newtons(&self) -> Vec<f64> {
        self.inner.force().to_vec()
    }

    fn time_at(&self, index: usize) -> PyResult<f64> {
        if index >= self.inner.len() {
            return Err(TrialError::new_err(format!(
                "sample index {index} is past the end of a trace of {} samples",
                self.inner.len()
            )));
        }
        Ok(self.inner.time_at(index))
    }

    fn __len__(&self) -> usize {
        self.inner.len()
    }

    fn __repr__(&self) -> String {
        format!(
            "Trial(sample_count={}, sample_rate_hz={}, duration_seconds={:.3}, acquisition_complete={}, reported_samples={})",
            self.inner.len(),
            self.inner.sample_rate_hz(),
            self.inner.duration_seconds(),
            if self.acquisition_complete() {
                "True"
            } else {
                "False"
            },
            self.exclusions.inner.dropped_samples
        )
    }
}

/// Reads a float64 buffer straight out of the object where one is offered, and falls back
/// to element-by-element conversion for a plain sequence.
///
/// A narrower array type is refused rather than widened. A float32 trace carries about
/// seven significant digits, and the impulse identity this software is checked against is
/// checked at 1e-9, so the widening has to be the caller's declared act.
fn read_float64_values(
    python: Python<'_>,
    values: &Bound<'_, PyAny>,
    argument: &str,
) -> PyResult<Vec<f64>> {
    if let Ok(buffer) = PyBuffer::<f64>::get(values) {
        if buffer.dimensions() != 1 {
            return Err(TrialError::new_err(format!(
                "{argument} has {} dimensions and a trace is one dimensional",
                buffer.dimensions()
            )));
        }
        // Reads straight into uninitialised capacity, so the trace is walked once rather
        // than zero-filled and then overwritten.
        return buffer.to_vec(python);
    }

    if values.hasattr("dtype").unwrap_or(false) {
        let dtype = values
            .getattr("dtype")
            .and_then(|d| d.str())
            .map(|d| d.to_string())
            .unwrap_or_else(|_| "unknown".to_string());
        return Err(TrialError::new_err(format!(
            "{argument} has dtype {dtype} and plateforce reads float64. Convert it with .astype('float64') so the widening is recorded as your choice"
        )));
    }

    values.extract::<Vec<f64>>().map_err(|_| {
        TrialError::new_err(format!(
            "{argument} must be a float64 buffer such as a numpy array, or a sequence of floats"
        ))
    })
}
