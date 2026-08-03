//! Batch from a notebook.
//!
//! The four relations arrive as lists of dictionaries and the converters are opt-in, so the
//! object pulls in no third-party package. Asking for one that is not installed says which
//! package would answer and what is available without it, rather than raising the import
//! error the caller cannot act on.

use std::path::PathBuf;

use plateforce_batch::{
    analyse, BatchRequest as CoreBatchRequest, SourceFormat, TrialIdentity as CoreTrialIdentity,
    TrialSet,
};
use plateforce_registry::Registry;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};

/// How a run names its trials.
///
/// Without a pattern a run has one unit of analysis, the trial. With one it also has a
/// subject, and every figure taken per athlete needs that.
#[pyclass(
    frozen,
    module = "plateforce",
    name = "TrialIdentity",
    skip_from_py_object
)]
#[derive(Clone)]
pub struct TrialIdentity {
    pub(crate) inner: CoreTrialIdentity,
}

#[pymethods]
impl TrialIdentity {
    /// The trial is named by its file stem and the run has no subject.
    #[staticmethod]
    fn file_stem() -> Self {
        Self {
            inner: CoreTrialIdentity::FileStem,
        }
    }

    /// A template such as `AT{subject}_{trial}`, which yields a subject as well.
    #[staticmethod]
    fn declared_pattern(template: &str) -> Self {
        Self {
            inner: CoreTrialIdentity::DeclaredPattern {
                template: template.to_string(),
            },
        }
    }

    fn __repr__(&self) -> String {
        format!("TrialIdentity({})", self.inner.describe())
    }
}

/// The run's own record: what it walked, what it read, and what identifies it.
#[pyclass(frozen, module = "plateforce", name = "BatchRun", skip_from_py_object)]
#[derive(Clone)]
pub struct BatchRun {
    pub(crate) row: plateforce_batch::RunRow,
}

#[pymethods]
impl BatchRun {
    #[getter]
    fn registry_digest(&self) -> &str {
        &self.row.registry_digest
    }
    #[getter]
    fn request_digest(&self) -> &str {
        &self.row.request_digest
    }
    #[getter]
    fn run_fingerprint(&self) -> &str {
        &self.row.run_fingerprint
    }
    #[getter]
    fn files_found(&self) -> usize {
        self.row.files_found
    }
    #[getter]
    fn trial_count(&self) -> usize {
        self.row.trial_count
    }
    #[getter]
    fn computed_count(&self) -> usize {
        self.row.computed_count
    }
    #[getter]
    fn refusal_count(&self) -> usize {
        self.row.refusal_count
    }
    #[getter]
    fn trials_excluded(&self) -> usize {
        self.row.trials_excluded
    }
    /// One means every trial was analysed the same way and produced the same quantities.
    #[getter]
    fn distinct_provenance_count(&self) -> usize {
        self.row.distinct_provenance_count
    }
    #[getter]
    fn trial_identity(&self) -> &str {
        &self.row.trial_identity
    }

    fn to_dict<'py>(&self, python: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        json_to_dict(python, &serde_json::to_value(&self.row).unwrap_or_default())
    }

    fn __repr__(&self) -> String {
        format!(
            "BatchRun(trials={} of {} found, computed={}, refused={})",
            self.row.trial_count,
            self.row.files_found,
            self.row.computed_count,
            self.row.refusal_count
        )
    }
}

/// The relations one run produced.
#[pyclass(
    frozen,
    module = "plateforce",
    name = "BatchResult",
    skip_from_py_object
)]
#[derive(Clone)]
pub struct BatchResult {
    pub(crate) inner: plateforce_batch::BatchResult,
}

#[pymethods]
impl BatchResult {
    /// One row per trial, one column per quantity.
    #[getter]
    fn results<'py>(&self, python: Python<'py>) -> PyResult<Bound<'py, PyList>> {
        rows(
            python,
            serde_json::to_value(&self.inner.results).unwrap_or_default(),
        )
    }

    /// One row per distinct chain per method per parameter.
    #[getter]
    fn provenance<'py>(&self, python: Python<'py>) -> PyResult<Bound<'py, PyList>> {
        rows(
            python,
            serde_json::to_value(&self.inner.provenance).unwrap_or_default(),
        )
    }

    /// One row per refusal, keyed by trial and ordinal, because a trial can decline one
    /// landmark and compute the rest at once.
    #[getter]
    fn refusals<'py>(&self, python: Python<'py>) -> PyResult<Bound<'py, PyList>> {
        rows(
            python,
            serde_json::to_value(&self.inner.refusals).unwrap_or_default(),
        )
    }

    #[getter]
    fn warnings<'py>(&self, python: Python<'py>) -> PyResult<Bound<'py, PyList>> {
        rows(
            python,
            serde_json::to_value(&self.inner.warnings).unwrap_or_default(),
        )
    }

    #[getter]
    fn aggregates<'py>(&self, python: Python<'py>) -> PyResult<Bound<'py, PyList>> {
        rows(
            python,
            serde_json::to_value(&self.inner.aggregates).unwrap_or_default(),
        )
    }

    #[getter]
    fn run(&self) -> BatchRun {
        BatchRun {
            row: self.inner.run.clone(),
        }
    }

    /// What the run walked, against the denominator each count is taken over.
    #[getter]
    fn coverage(&self) -> String {
        self.inner.coverage.line()
    }

    fn to_pylist<'py>(&self, python: Python<'py>) -> PyResult<Bound<'py, PyList>> {
        rows(
            python,
            serde_json::to_value(&self.inner.results).unwrap_or_default(),
        )
    }

    /// The same string the browser and the library produce for the same input.
    fn to_json(&self) -> String {
        self.inner.to_json()
    }

    fn write_csv(&self, directory: PathBuf) -> PyResult<Vec<String>> {
        self.inner
            .write_csv(&directory)
            .map(|paths| paths.iter().map(|p| p.display().to_string()).collect())
            .map_err(|error| PyValueError::new_err(error.to_string()))
    }

    #[cfg(feature = "parquet")]
    fn write_parquet(&self, directory: PathBuf) -> PyResult<Vec<String>> {
        self.inner
            .write_parquet(&directory)
            .map(|paths| paths.iter().map(|p| p.display().to_string()).collect())
            .map_err(|error| PyValueError::new_err(error.to_string()))
    }

    /// Arrow and pandas are not dependencies of this package. Asking for one that is absent
    /// says which package answers and what is available without it.
    fn to_arrow(&self, python: Python<'_>) -> PyResult<Py<PyAny>> {
        convert_through(python, "pyarrow", "to_pylist")
    }

    fn to_pandas(&self, python: Python<'_>) -> PyResult<Py<PyAny>> {
        convert_through(python, "pandas", "to_pylist")
    }

    /// A frozen class cannot take `__setstate__`, so the whole object travels as the one
    /// string every surface already agrees on. Without this a pool over a directory could
    /// not return its results.
    fn __reduce__(&self, python: Python<'_>) -> PyResult<(Py<PyAny>, (String,))> {
        let module = python.import("plateforce")?;
        let rebuild = module.getattr("_batch_result_from_json")?;
        Ok((rebuild.unbind(), (self.inner.to_json(),)))
    }

    fn __repr__(&self) -> String {
        format!("BatchResult({})", self.inner.coverage.line())
    }
}

/// Rebuild a result from the envelope, for pickle.
#[pyfunction]
#[pyo3(name = "_batch_result_from_json")]
pub fn batch_result_from_json(text: &str) -> PyResult<BatchResult> {
    plateforce_batch::BatchResult::from_json(text)
        .map(|inner| BatchResult { inner })
        .map_err(PyValueError::new_err)
}

/// Run one analysis over every trial under a directory.
///
/// `sentinel` is the value this export writes where a sample is missing, or `None` to state
/// that it writes none. It is keyword-only and undefaulted, so omitting it raises rather than
/// reading a vendor's missing marker as a force.
#[pyfunction]
#[pyo3(signature = (directory, *, registry, weighing, onset, takeoff, sentinel, delimiter = "\t", force_column_index = 0, sample_rate_hz = 1000.0, trial_file_suffixes = None, pattern = None, resolved = None))]
#[allow(clippy::too_many_arguments)]
pub fn batch(
    directory: PathBuf,
    registry: PathBuf,
    weighing: &str,
    onset: &str,
    takeoff: &str,
    sentinel: Option<f64>,
    delimiter: &str,
    force_column_index: usize,
    sample_rate_hz: f64,
    trial_file_suffixes: Option<Vec<String>>,
    pattern: Option<&str>,
    resolved: Option<Vec<String>>,
) -> PyResult<BatchResult> {
    let delimiter = delimiter
        .chars()
        .next()
        .ok_or_else(|| PyValueError::new_err("a delimiter is one character"))?;
    // No default: a walk that filtered quietly would drop files out of the denominator with
    // nothing recording it.
    let suffixes = trial_file_suffixes.ok_or_else(|| {
        PyValueError::new_err(
            "a run declares which file names are trials, so that the count it reports has a denominator",
        )
    })?;

    let format = SourceFormat {
        delimiter,
        force_column_index,
        sample_rate_hz,
        trial_file_suffixes: suffixes,
        sentinel,
    };
    let identity = match pattern {
        Some(template) => CoreTrialIdentity::DeclaredPattern {
            template: template.to_string(),
        },
        None => CoreTrialIdentity::FileStem,
    };

    let set = TrialSet::walk(&directory, &format, &identity)
        .map_err(|error| PyValueError::new_err(error.to_string()))?;
    let loaded =
        Registry::load(&registry).map_err(|error| PyValueError::new_err(error.to_string()))?;

    let analysis = plateforce_analysis::AnalysisRequest {
        weighing: plateforce_analysis::WeighingChoice {
            method_id: weighing.to_string(),
            ..Default::default()
        },
        onset: plateforce_analysis::MethodChoice {
            method_id: onset.to_string(),
            ..Default::default()
        },
        takeoff: plateforce_analysis::MethodChoice {
            method_id: takeoff.to_string(),
            ..Default::default()
        },
        touchdown_index: None,
        gravity_meters_per_second_squared:
            plateforce_core::STANDARD_GRAVITY_METERS_PER_SECOND_SQUARED,
        registry_backed_ids: Vec::new(),
        ..Default::default()
    };

    let declared = resolved.unwrap_or_default();
    let borrowed: Vec<&str> = declared.iter().map(String::as_str).collect();
    let request = CoreBatchRequest::new(analysis).resolving(&borrowed);

    analyse(&set, &request, &loaded)
        .map(|inner| BatchResult { inner })
        .map_err(|refusal| PyValueError::new_err(refusal.message))
}

/// One dictionary per row, from a value the caller already serialised.
fn rows<'py>(python: Python<'py>, value: serde_json::Value) -> PyResult<Bound<'py, PyList>> {
    let list = PyList::empty(python);
    for entry in value.as_array().cloned().unwrap_or_default() {
        list.append(json_to_dict(python, &entry)?)?;
    }
    Ok(list)
}

fn json_to_dict<'py>(
    python: Python<'py>,
    value: &serde_json::Value,
) -> PyResult<Bound<'py, PyDict>> {
    let dictionary = PyDict::new(python);
    if let Some(object) = value.as_object() {
        for (key, entry) in object {
            dictionary.set_item(key, json_to_object(python, entry)?)?;
        }
    }
    Ok(dictionary)
}

fn json_to_object(python: Python<'_>, value: &serde_json::Value) -> PyResult<Py<PyAny>> {
    Ok(match value {
        serde_json::Value::Null => python.None(),
        serde_json::Value::Bool(flag) => flag.into_pyobject(python)?.to_owned().into(),
        serde_json::Value::Number(number) => match number.as_f64() {
            Some(figure) => figure.into_pyobject(python)?.into(),
            None => python.None(),
        },
        serde_json::Value::String(text) => text.into_pyobject(python)?.into(),
        serde_json::Value::Array(items) => {
            let list = PyList::empty(python);
            for item in items {
                list.append(json_to_object(python, item)?)?;
            }
            list.into()
        }
        serde_json::Value::Object(_) => json_to_dict(python, value)?.into(),
    })
}

fn convert_through(python: Python<'_>, package: &str, available: &str) -> PyResult<Py<PyAny>> {
    match python.import(package) {
        Ok(_) => Err(PyValueError::new_err(format!(
            "{package} is installed; call .{available}() and hand the rows to it"
        ))),
        Err(_) => Err(PyValueError::new_err(format!(
            "{package} answers this, and .{available}() returns the same rows without it"
        ))),
    }
}
