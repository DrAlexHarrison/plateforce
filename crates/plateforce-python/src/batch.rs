//! Batch from a notebook.
//!
//! The relations arrive as lists of dictionaries. The table converters are opt-in and read
//! that same results relation, so changing representation cannot change which trials leave.

use std::path::PathBuf;

use plateforce_batch::{
    analyse, with_aggregates, AggregationRequest, BatchRequest as CoreBatchRequest, GroupKind,
    SourceFormat, TrialIdentity as CoreTrialIdentity, TrialSet,
};
use plateforce_core::DispersionEstimator;
use plateforce_registry::Registry;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};

use crate::trial::Acquisition;

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
    /// The digest over this run, and `None` when the acquisition block was not filled.
    ///
    /// `None` rather than a marked string, because a run whose plate settings nobody recorded
    /// cannot be declared to match another and any value here would compare equal to the next
    /// such run's. `acquisition_complete` says whether one was published.
    #[getter]
    fn run_fingerprint(&self) -> Option<&str> {
        self.row.run_fingerprint.as_deref()
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

    /// One row per number the run produced, carrying the account that number gives of
    /// itself. Keyed by trial and quantity, because an account opens with its own value and
    /// two trials that ran identically still give different accounts of themselves.
    #[getter]
    fn descriptions<'py>(&self, python: Python<'py>) -> PyResult<Bound<'py, PyList>> {
        rows(
            python,
            serde_json::to_value(&self.inner.descriptions).unwrap_or_default(),
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

    /// What the analysis already knew about the numbers it reported, per trial. A refusal
    /// means no number was produced; a signal qualifies numbers that were.
    #[getter]
    fn signals<'py>(&self, python: Python<'py>) -> PyResult<Bound<'py, PyList>> {
        rows(
            python,
            serde_json::to_value(&self.inner.signals).unwrap_or_default(),
        )
    }

    /// What each bound gate found, whether or not the request asked it to remove anything.
    #[getter]
    fn exclusions<'py>(&self, python: Python<'py>) -> PyResult<Bound<'py, PyList>> {
        rows(
            python,
            serde_json::to_value(&self.inner.exclusions).unwrap_or_default(),
        )
    }

    /// The quantity columns, in the order the analysis reported them, and the unit each is
    /// in as the registry spells it. Carried rather than read off a column name.
    #[getter]
    fn quantities(&self) -> Vec<String> {
        self.inner.quantities.clone()
    }

    #[getter]
    fn units(&self) -> std::collections::BTreeMap<String, String> {
        self.inner.units.clone()
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

    /// One Arrow row per trial, with the same columns and order as `to_pylist()`.
    fn to_arrow(&self, python: Python<'_>) -> PyResult<Py<PyAny>> {
        let rows = self.to_pylist(python)?;
        let arrow = python.import("pyarrow").map_err(|_| {
            PyValueError::new_err(
                "one dictionary per trial is available through .to_pylist(); install pyarrow to read those rows as an Arrow table",
            )
        })?;
        arrow
            .getattr("Table")?
            .call_method1("from_pylist", (rows,))
            .map(Bound::unbind)
    }

    /// One DataFrame row per trial, with the same columns and order as `to_pylist()`.
    fn to_pandas(&self, python: Python<'_>) -> PyResult<Py<PyAny>> {
        let rows = self.to_pylist(python)?;
        let pandas = python.import("pandas").map_err(|_| {
            PyValueError::new_err(
                "one dictionary per trial is available through .to_pylist(); install pandas to read those rows as a DataFrame",
            )
        })?;
        pandas
            .getattr("DataFrame")?
            .call1((rows,))
            .map(Bound::unbind)
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

/// What one folder run says about the phase that conditions the signal, keyed by the construct
/// the registry declares.
///
/// The keys are the union of the three arguments, because a caller may name a rule for the
/// phase, state values against the rule it runs anyway, or both. A construct none of them
/// names is left out: the phase runs it either way and leaves the same record, so a key in the
/// map is the caller having spoken.
fn conditioning_choices(
    rules: &std::collections::BTreeMap<String, String>,
    parameters: &std::collections::BTreeMap<String, std::collections::BTreeMap<String, f64>>,
    options: &std::collections::BTreeMap<String, std::collections::BTreeMap<String, String>>,
) -> std::collections::BTreeMap<String, plateforce_analysis::MethodChoice> {
    let mut constructs: std::collections::BTreeSet<&String> = std::collections::BTreeSet::new();
    constructs.extend(rules.keys());
    constructs.extend(parameters.keys());
    constructs.extend(options.keys());
    constructs
        .into_iter()
        .map(|construct| {
            (
                construct.clone(),
                plateforce_analysis::MethodChoice {
                    method_id: rules.get(construct).cloned().unwrap_or_default(),
                    parameters: parameters.get(construct).cloned().unwrap_or_default(),
                    options: options.get(construct).cloned().unwrap_or_default(),
                    ..Default::default()
                },
            )
        })
        .collect()
}

/// Run one analysis over every trial under a directory.
///
/// `sentinel` is the value this export writes where a sample is missing, or `None` to state
/// that it writes none. It is keyword-only and undefaulted, so omitting it raises rather than
/// reading a vendor's missing marker as a force.
///
/// `sample_rate_hz` is also keyword-only and undefaulted. The files do not carry it, and a
/// guessed rate moves velocity and impulse once and height and displacement twice.
///
/// `acquisition` records all 5 of 5 capture facts that are not already carried by the sample
/// rate. A complete block gives the run a fingerprint; a partial block remains on the run and
/// keeps the fingerprint empty rather than declaring incomplete captures to match.
///
/// `aggregate` names the published rule that reduces an athlete's trials to one number.
/// `trial.aggregation` publishes three and none of them is the arithmetic mean of a session, so
/// naming none leaves the run unreduced and naming one the registry does not publish is refused
/// by name. `aggregate_n` is the trial count the rule was asked for and travels with the value
/// everywhere it is reported, because best of five and best of three are different numbers.
/// `aggregate_ranked_by` names the registry construct whose value orders the trials. A rule
/// whose name does not carry its own criterion refuses when this is absent.
/// `aggregate_by` reduces over subject, session or run. `aggregate_quantity` scopes which
/// quantities are reduced, defaulting to every quantity the run computed, which is a scope
/// rather than a method choice because each row names what it reduced.
#[pyfunction]
#[pyo3(signature = (directory, *, registry, weighing, onset, takeoff, sentinel, sample_rate_hz, delimiter = "\t", force_column_index = 0, acquisition = None, trial_file_suffixes = None, pattern = None, resolved = None, weighing_parameters = None, onset_parameters = None, takeoff_parameters = None, weighing_options = None, onset_options = None, takeoff_options = None, derived = None, derived_parameters = None, derived_options = None, conditioning = None, conditioning_parameters = None, conditioning_options = None, gravity_meters_per_second_squared = None, body_mass_kilograms = None, aggregate = None, aggregate_n = None, aggregate_ranked_by = None, aggregate_by = "subject", aggregate_quantity = None, aggregate_dispersion = "sample"))]
#[allow(clippy::too_many_arguments)]
pub fn batch(
    directory: PathBuf,
    registry: PathBuf,
    weighing: &str,
    onset: &str,
    takeoff: &str,
    sentinel: Option<f64>,
    sample_rate_hz: f64,
    delimiter: &str,
    force_column_index: usize,
    acquisition: Option<Acquisition>,
    trial_file_suffixes: Option<Vec<String>>,
    pattern: Option<&str>,
    resolved: Option<Vec<String>>,
    weighing_parameters: Option<std::collections::BTreeMap<String, f64>>,
    onset_parameters: Option<std::collections::BTreeMap<String, f64>>,
    takeoff_parameters: Option<std::collections::BTreeMap<String, f64>>,
    weighing_options: Option<std::collections::BTreeMap<String, String>>,
    onset_options: Option<std::collections::BTreeMap<String, String>>,
    takeoff_options: Option<std::collections::BTreeMap<String, String>>,
    derived: Option<std::collections::BTreeMap<String, String>>,
    derived_parameters: Option<
        std::collections::BTreeMap<String, std::collections::BTreeMap<String, f64>>,
    >,
    derived_options: Option<
        std::collections::BTreeMap<String, std::collections::BTreeMap<String, String>>,
    >,
    conditioning: Option<std::collections::BTreeMap<String, String>>,
    conditioning_parameters: Option<
        std::collections::BTreeMap<String, std::collections::BTreeMap<String, f64>>,
    >,
    conditioning_options: Option<
        std::collections::BTreeMap<String, std::collections::BTreeMap<String, String>>,
    >,
    gravity_meters_per_second_squared: Option<f64>,
    body_mass_kilograms: Option<f64>,
    aggregate: Option<String>,
    aggregate_n: Option<usize>,
    aggregate_ranked_by: Option<String>,
    aggregate_by: &str,
    aggregate_quantity: Option<Vec<String>>,
    aggregate_dispersion: &str,
) -> PyResult<BatchResult> {
    let body_mass_kilograms = Python::attach(|python| {
        crate::analysis::stated_body_mass(body_mass_kilograms)
            .map_err(|refusal| crate::errors::raise_refusal(python, &refusal))
    })?;
    // The value and the claim about where it came from are written together, by the one
    // routine every surface writes a gravity through.
    let (gravity_meters_per_second_squared, gravity_source) =
        plateforce_analysis::gravity_stated(gravity_meters_per_second_squared);
    let delimiter =
        Python::attach(|python| crate::trial::field_separator(python, "batch", delimiter))?;
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

    // A construct computed from the landmarks has no argument of its own, so the rule, its
    // values and its names are keyed by the construct.
    let derived = derived.unwrap_or_default();
    let derived_parameters = derived_parameters.unwrap_or_default();
    let derived_options = derived_options.unwrap_or_default();

    // The phase that conditions the signal runs on every trial in the folder, so a value
    // written against it applies to every trial the way a landmark rule's does. Keyed the same
    // way, and checked through the predicate the engine checks a request with, so a folder run
    // and a single trial cannot report one recording as conditioned two ways.
    let conditioning = conditioning_choices(
        &conditioning.unwrap_or_default(),
        &conditioning_parameters.unwrap_or_default(),
        &conditioning_options.unwrap_or_default(),
    );
    for (construct, choice) in &conditioning {
        plateforce_analysis::binding::accepts_conditioning(construct, &choice.method_id)
            .map_err(|refusal| PyValueError::new_err(refusal.message().to_string()))?;
    }
    let analysis = plateforce_analysis::AnalysisRequest {
        weighing: plateforce_analysis::WeighingChoice {
            method_id: weighing.to_string(),
            parameters: weighing_parameters.unwrap_or_default(),
            options: weighing_options.unwrap_or_default(),
            ..Default::default()
        },
        onset: plateforce_analysis::MethodChoice {
            method_id: onset.to_string(),
            parameters: onset_parameters.unwrap_or_default(),
            options: onset_options.unwrap_or_default(),
            ..Default::default()
        },
        takeoff: plateforce_analysis::MethodChoice {
            method_id: takeoff.to_string(),
            parameters: takeoff_parameters.unwrap_or_default(),
            options: takeoff_options.unwrap_or_default(),
            ..Default::default()
        },
        touchdown_index: None,
        gravity_meters_per_second_squared,
        gravity_source,
        // Stated once for the folder, as the plate and the acquisition block are: every file
        // in one folder came off one athlete on one day.
        body_mass_kilograms,
        // What this registry carries, so a rule the registry files is recorded as backed
        // rather than as the run's own. A list built from the caller's choices alone reports
        // the operators a binding composes as absent from the registry they are filed in.
        registry_backed_ids: loaded.methods.keys().cloned().collect(),
        conditioning,
        derived: derived
            .iter()
            .map(|(construct, method_id)| {
                (
                    construct.clone(),
                    plateforce_analysis::MethodChoice {
                        method_id: method_id.clone(),
                        parameters: derived_parameters
                            .get(construct)
                            .cloned()
                            .unwrap_or_default(),
                        options: derived_options.get(construct).cloned().unwrap_or_default(),
                        ..Default::default()
                    },
                )
            })
            .collect(),
    };
    let mut analysis = analysis;
    // The published defaults every trial in the folder runs on, read from the registry this
    // call loaded rather than held in the rules.
    analysis.reading(&loaded);

    let declared = resolved.unwrap_or_default();
    let borrowed: Vec<&str> = declared.iter().map(String::as_str).collect();
    let request = CoreBatchRequest::new(analysis).resolving(&borrowed);
    let request = match acquisition {
        Some(acquisition) => request.describing(acquisition.block()),
        None => request,
    };

    let result = analyse(&set, &request, &loaded)
        .map_err(|refusal| PyValueError::new_err(refusal.message))?;

    // Naming no rule leaves the run unreduced, which is what `aggregates` reported on every
    // call this surface could make before it could reduce at all. Naming one binds
    // `trial.aggregation`, which publishes three incompatible rules and refuses rather than
    // taking a mean, so the refusals below are the feature and not the edge case.
    if aggregate.is_none()
        && aggregate_n.is_none()
        && aggregate_ranked_by.is_none()
        && aggregate_quantity.is_none()
    {
        return Ok(BatchResult { inner: result });
    }

    let group_kind = match aggregate_by {
        "subject" => GroupKind::Subject,
        "session" => GroupKind::Session,
        "run" => GroupKind::Run,
        other => {
            return Err(PyValueError::new_err(format!(
                "a reduction is taken over subject, session or run, and this one named {other}"
            )))
        }
    };

    // Read through core's own words rather than matched here, so a third estimator arrives on
    // this argument without an edit and cannot arrive under a second spelling.
    let dispersion = DispersionEstimator::from_published_str(aggregate_dispersion).ok_or_else(
        || {
            PyValueError::new_err(format!(
                "the standard deviation beside a reduced value is one of {}, and this run named {aggregate_dispersion}",
                DispersionEstimator::PUBLISHED.join(", "),
            ))
        },
    )?;

    // Every quantity the run computed, where nobody named one. A scope rather than a method
    // choice, and each row names the quantity it reduced, so nothing is reduced unseen.
    let quantities = match aggregate_quantity {
        None => result.quantities.clone(),
        Some(named) => {
            let absent: Vec<&str> = named
                .iter()
                .filter(|key| !result.quantities.contains(key))
                .map(String::as_str)
                .collect();
            if !absent.is_empty() {
                return Err(PyValueError::new_err(format!(
                    "this run computed {}, and a reduction was asked for {}",
                    result.quantities.join(", "),
                    absent.join(", ")
                )));
            }
            named
        }
    };

    let reduction = AggregationRequest::declared(
        aggregate.as_deref(),
        aggregate_n,
        aggregate_ranked_by.as_deref(),
        group_kind,
        quantities,
        dispersion,
    )
    .map_err(|refusal| PyValueError::new_err(refusal.against(&result).message()))?;

    with_aggregates(result, &set, &reduction)
        .map(|inner| BatchResult { inner })
        .map_err(|refusal| PyValueError::new_err(refusal.message()))
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
