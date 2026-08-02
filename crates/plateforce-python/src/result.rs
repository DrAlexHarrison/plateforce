//! What a result carries besides its number.
//!
//! `Measured` deliberately has no `__float__`. A number that can slip into arithmetic
//! without its method attached is the failure this package exists to make impossible, so
//! reaching the bare value is an explicit `.value`.

use plateforce_core::reporting::{describe, format_parameters};
use plateforce_core::{Exclusions as CoreExclusions, Measured as CoreMeasured};
use pyo3::prelude::*;
use pyo3::types::PyDict;

/// The chain lives in the core, where every surface can reach it. R links the engine
/// crates and cannot link this extension module.
pub use plateforce_core::ProvenanceChain;

#[pyclass(
    frozen,
    skip_from_py_object,
    module = "plateforce",
    name = "Provenance"
)]
#[derive(Clone)]
pub struct Provenance {
    pub(crate) chain: ProvenanceChain,
}

#[pymethods]
impl Provenance {
    /// Canonical dotted registry id of the method that produced the value.
    #[getter]
    fn method_id(&self) -> &str {
        &self.chain.provenance.method_id
    }

    /// The parameter values this method was bound to, defaults included.
    #[getter]
    fn bound_parameters<'py>(&self, python: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let bound = PyDict::new(python);
        for (name, value) in &self.chain.provenance.bound_parameters {
            bound.set_item(name, value)?;
        }
        Ok(bound)
    }

    /// The revision the caller pinned, and None when they pinned none.
    #[getter]
    fn registry_version(&self) -> Option<&str> {
        self.chain.provenance.registry_version.as_deref()
    }

    /// Identifies the registry files this value was computed against, measured from their
    /// bytes.
    #[getter]
    fn registry_digest(&self) -> Option<&str> {
        self.chain.provenance.registry_digest.as_deref()
    }

    /// False when the acquisition block could not be filled. A result with this false
    /// must never be declared to match another lab's, however well the analysis matches.
    #[getter]
    fn acquisition_complete(&self) -> bool {
        self.chain.provenance.acquisition_complete
    }

    /// Choices between named alternatives, which move the number as much as the numeric
    /// parameters do. Population against sample standard deviation is one of them.
    #[getter]
    fn enumerated_choices<'py>(&self, python: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let chosen = PyDict::new(python);
        for (name, value) in &self.chain.enumerated_choices {
            chosen.set_item(name, value)?;
        }
        Ok(chosen)
    }

    /// Provenance of each result this one was computed from.
    #[getter]
    fn depends_on(&self) -> Vec<Provenance> {
        self.chain
            .depends_on
            .iter()
            .map(|chain| Provenance {
                chain: chain.clone(),
            })
            .collect()
    }

    /// This provenance and every one upstream of it, in one list.
    ///
    /// The parameter that moved a downstream number usually sits on an upstream step: the
    /// k that placed onset is on the onset entry, not on the time to takeoff derived from it.
    fn flattened(&self) -> Vec<Provenance> {
        self.chain
            .flattened()
            .into_iter()
            .map(|step| Provenance {
                chain: step.clone(),
            })
            .collect()
    }

    /// The parameters bound to a named method anywhere in this chain, or None when the
    /// chain does not include it.
    fn parameters_of<'py>(
        &self,
        python: Python<'py>,
        method_id: &str,
    ) -> PyResult<Option<Bound<'py, PyDict>>> {
        match self.chain.step_of(method_id) {
            Some(step) => Provenance {
                chain: step.clone(),
            }
            .bound_parameters(python)
            .map(Some),
            None => Ok(None),
        }
    }

    fn __repr__(&self) -> String {
        format!(
            "Provenance(method_id='{}', bound_parameters={}, registry_version={}, registry_digest={}, acquisition_complete={})",
            self.chain.provenance.method_id,
            format_parameters(&self.chain.provenance.bound_parameters),
            optional(self.chain.provenance.registry_version.as_deref()),
            optional(self.chain.provenance.registry_digest.as_deref()),
            if self.chain.provenance.acquisition_complete {
                "True"
            } else {
                "False"
            }
        )
    }
}

/// A value, its unit, and the choices that produced it.
#[pyclass(frozen, skip_from_py_object, module = "plateforce", name = "Measured")]
#[derive(Clone)]
pub struct Measured {
    pub(crate) inner: CoreMeasured,
    pub(crate) enumerated_choices: Vec<(String, String)>,
    pub(crate) depends_on: Vec<ProvenanceChain>,
}

impl Measured {
    pub fn new(
        inner: CoreMeasured,
        enumerated_choices: Vec<(String, String)>,
        depends_on: Vec<ProvenanceChain>,
    ) -> Self {
        Self {
            inner,
            enumerated_choices,
            depends_on,
        }
    }

    pub fn chain(&self) -> ProvenanceChain {
        ProvenanceChain {
            provenance: self.inner.provenance.clone(),
            enumerated_choices: self.enumerated_choices.clone(),
            depends_on: self.depends_on.clone(),
        }
    }

    pub fn value_for_display(&self) -> f64 {
        self.inner.value
    }
}

#[pymethods]
impl Measured {
    /// The bare number, in `unit`. Reaching for this drops the provenance, which is why
    /// it is a named attribute and not an implicit float conversion.
    #[getter]
    fn value(&self) -> f64 {
        self.inner.value
    }

    #[getter]
    fn unit(&self) -> &str {
        self.inner.unit
    }

    #[getter]
    fn provenance(&self) -> Provenance {
        Provenance {
            chain: self.chain(),
        }
    }

    /// Multi-line account of the value and every choice behind it, upstream steps included.
    fn describe(&self) -> String {
        describe(&self.inner, &self.chain())
    }

    fn __repr__(&self) -> String {
        format!(
            "Measured(value={}, unit='{}', method_id='{}')",
            self.inner.value, self.inner.unit, self.inner.provenance.method_id
        )
    }

    fn __str__(&self) -> String {
        format!(
            "{} {} by {}",
            self.inner.value, self.inner.unit, self.inner.provenance.method_id
        )
    }
}

/// What a step dropped, and under which rule.
#[pyclass(
    frozen,
    skip_from_py_object,
    module = "plateforce",
    name = "Exclusions"
)]
#[derive(Clone)]
pub struct Exclusions {
    pub(crate) inner: CoreExclusions,
}

#[pymethods]
impl Exclusions {
    #[getter]
    fn dropped_samples(&self) -> usize {
        self.inner.dropped_samples
    }

    #[getter]
    fn reason(&self) -> Option<&str> {
        self.inner.reason.as_deref()
    }

    /// The sentinel convention that was applied, or None when none was declared.
    #[getter]
    fn sentinel_convention(&self) -> Option<&str> {
        self.inner.sentinel_convention.as_deref()
    }

    fn __repr__(&self) -> String {
        format!(
            "Exclusions(dropped_samples={}, sentinel_convention={}, reason={})",
            self.inner.dropped_samples,
            optional(self.inner.sentinel_convention.as_deref()),
            optional(self.inner.reason.as_deref())
        )
    }
}

fn optional(value: Option<&str>) -> String {
    match value {
        Some(text) => format!("'{text}'"),
        None => "None".to_string(),
    }
}
