//! What a result carries besides its number.
//!
//! `Measured` deliberately has no `__float__`. A number that can slip into arithmetic
//! without its method attached is the failure this package exists to make impossible, so
//! reaching the bare value is an explicit `.value`.

use plateforce_core::{Exclusions as CoreExclusions, Measured as CoreMeasured, Provenance as CoreProvenance};
use pyo3::prelude::*;
use pyo3::types::PyDict;

/// A provenance and the provenances of the results it was computed from.
///
/// Jump height moves with the onset rule and the weighing epoch as well as with the
/// jump-height formula, so a result that named only the last step would understate what
/// produced it. Core's `Provenance` has no field for upstream steps, so the chain is
/// assembled here around core values rather than replacing them.
#[derive(Clone)]
pub struct ProvenanceChain {
    pub provenance: CoreProvenance,
    pub depends_on: Vec<ProvenanceChain>,
}

impl ProvenanceChain {
    pub fn leaf(provenance: CoreProvenance) -> Self {
        Self {
            provenance,
            depends_on: Vec::new(),
        }
    }

    pub fn with_inputs(provenance: CoreProvenance, depends_on: Vec<ProvenanceChain>) -> Self {
        Self {
            provenance,
            depends_on,
        }
    }
}

#[pyclass(frozen, module = "plateforce", name = "Provenance")]
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

    #[getter]
    fn registry_version(&self) -> &str {
        &self.chain.provenance.registry_version
    }

    /// False when the acquisition block could not be filled. A result with this false
    /// must never be declared to match another lab's, however well the analysis matches.
    #[getter]
    fn acquisition_complete(&self) -> bool {
        self.chain.provenance.acquisition_complete
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

    fn __repr__(&self) -> String {
        format!(
            "Provenance(method_id='{}', bound_parameters={}, registry_version='{}', acquisition_complete={})",
            self.chain.provenance.method_id,
            format_parameters(&self.chain.provenance.bound_parameters),
            self.chain.provenance.registry_version,
            if self.chain.provenance.acquisition_complete {
                "True"
            } else {
                "False"
            }
        )
    }
}

fn format_parameters(parameters: &[(String, f64)]) -> String {
    if parameters.is_empty() {
        return "{}".to_string();
    }
    let body = parameters
        .iter()
        .map(|(name, value)| format!("'{name}': {value}"))
        .collect::<Vec<_>>()
        .join(", ");
    format!("{{{body}}}")
}

/// A value, its unit, and the choices that produced it.
#[pyclass(frozen, module = "plateforce", name = "Measured")]
#[derive(Clone)]
pub struct Measured {
    pub(crate) inner: CoreMeasured,
    pub(crate) depends_on: Vec<ProvenanceChain>,
}

impl Measured {
    pub fn new(inner: CoreMeasured, depends_on: Vec<ProvenanceChain>) -> Self {
        Self { inner, depends_on }
    }

    pub fn chain(&self) -> ProvenanceChain {
        ProvenanceChain::with_inputs(self.inner.provenance.clone(), self.depends_on.clone())
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
        let mut lines = vec![format!(
            "{} {}",
            self.inner.value, self.inner.unit
        )];
        describe_chain(&self.chain(), 0, &mut lines);
        if !self.inner.provenance.acquisition_complete {
            lines.push(
                "  acquisition block incomplete, so this result cannot be declared to match another lab's"
                    .to_string(),
            );
        }
        lines.join("\n")
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

fn describe_chain(chain: &ProvenanceChain, depth: usize, lines: &mut Vec<String>) {
    let indent = "  ".repeat(depth + 1);
    lines.push(format!(
        "{indent}{} {}",
        chain.provenance.method_id,
        format_parameters(&chain.provenance.bound_parameters)
    ));
    if depth == 0 {
        lines.push(format!(
            "{indent}registry {}",
            chain.provenance.registry_version
        ));
    }
    for input in &chain.depends_on {
        describe_chain(input, depth + 1, lines);
    }
}

/// What a step dropped, and under which rule.
#[pyclass(frozen, module = "plateforce", name = "Exclusions")]
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
