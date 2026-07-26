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
    /// Choices that select between named alternatives rather than between numbers, such
    /// as population against sample standard deviation. Core's `bound_parameters` is a
    /// list of `(String, f64)` and cannot hold one, so they are carried here.
    pub enumerated_choices: Vec<(String, String)>,
    pub depends_on: Vec<ProvenanceChain>,
}

impl ProvenanceChain {
    pub fn leaf(provenance: CoreProvenance) -> Self {
        Self {
            provenance,
            enumerated_choices: Vec::new(),
            depends_on: Vec::new(),
        }
    }

    pub fn with_inputs(provenance: CoreProvenance, depends_on: Vec<ProvenanceChain>) -> Self {
        Self {
            provenance,
            enumerated_choices: Vec::new(),
            depends_on,
        }
    }

    pub fn choosing(mut self, choices: Vec<(String, String)>) -> Self {
        self.enumerated_choices = choices;
        self
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
    /// k that placed onset is on the onset entry, not on the time to takeoff derived from
    /// it. Walking the tree by hand to find it is a chore, so this flattens it.
    fn flattened(&self) -> Vec<Provenance> {
        let mut collected = Vec::new();
        collect(&self.chain, &mut collected);
        collected
    }

    /// The parameters bound to a named method anywhere in this chain, or None when the
    /// chain does not include it.
    fn parameters_of<'py>(
        &self,
        python: Python<'py>,
        method_id: &str,
    ) -> PyResult<Option<Bound<'py, PyDict>>> {
        let mut collected = Vec::new();
        collect(&self.chain, &mut collected);
        for provenance in collected {
            if provenance.chain.provenance.method_id == method_id {
                return provenance.bound_parameters(python).map(Some);
            }
        }
        Ok(None)
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

fn collect(chain: &ProvenanceChain, into: &mut Vec<Provenance>) {
    into.push(Provenance {
        chain: chain.clone(),
    });
    for input in &chain.depends_on {
        collect(input, into);
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
    for (name, value) in &chain.enumerated_choices {
        lines.push(format!("{indent}  {name} = {value}"));
    }
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
