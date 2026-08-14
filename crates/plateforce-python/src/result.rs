//! What a result carries besides its number.
//!
//! `Measured` has no `__float__`. Reaching the bare number is an explicit `.value`, so a
//! value cannot slip into arithmetic without its method attached.

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

/// The word this surface reports for each source, matched exhaustively so a source added to
/// the vocabulary has to be ruled on here rather than reaching Python unnamed.
fn source_name(source: plateforce_core::provenance::ParameterSource) -> &'static str {
    use plateforce_core::provenance::ParameterSource::*;
    match source {
        Stated => "stated",
        Assumed => "assumed",
        Measured => "measured",
        Recommended => "recommended",
        Provisional => "provisional",
        Cited => "cited",
    }
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
        for (name, value) in &self.chain.provenance.bound_parameters() {
            bound.set_item(name, value)?;
        }
        Ok(bound)
    }

    /// The published pipeline this rule and its cited values were adopted from, and None on
    /// a step no pipeline spoke to. A pipeline binds the constructs its source states, so a
    /// result carries it on some steps and not others.
    #[getter]
    fn preset(&self) -> Option<&str> {
        self.chain
            .provenance
            .preset
            .as_ref()
            .map(|adopted| adopted.id.as_str())
    }

    /// Values the pipeline states for this rule that the caller replaced. What ran is in
    /// `bound_parameters`, and this is what the source published for the same names.
    #[getter]
    fn superseded_by_caller<'py>(&self, python: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let displaced = PyDict::new(python);
        if let Some(adopted) = &self.chain.provenance.preset {
            for (name, value) in &adopted.superseded_parameters {
                displaced.set_item(name, value)?;
            }
            for (name, value) in &adopted.superseded_options {
                displaced.set_item(name, value)?;
            }
        }
        Ok(displaced)
    }

    /// Where the rule itself came from: stated by the caller, accepted from the registry's
    /// recommendation, adopted with a published pipeline, or assumed, which is the rule the
    /// registry declares for a construct nobody named. The four move the number identically
    /// and answer different questions a methods section asks.
    #[getter]
    fn method_source(&self) -> &'static str {
        source_name(self.chain.provenance.method_source)
    }

    /// The revision the caller pinned, and None when they pinned none.
    #[getter]
    fn registry_version(&self) -> Option<&str> {
        self.chain.provenance.registry_version.as_deref()
    }

    /// The revision the registry names about itself, and None where it names none.
    ///
    /// Distinct from `registry_version` above, which is what the caller pinned. Either can be
    /// present without the other: a caller can pin a revision the registry does not claim,
    /// and a registry can claim one nobody pinned. `Registry.declared_version` answers the
    /// same question about a registry a notebook is holding; this answers it about the
    /// registry that produced this number.
    #[getter]
    fn registry_declared_version(&self) -> Option<&str> {
        self.chain.provenance.registry_declared_version.as_deref()
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

    /// True where a reader placed this rule's landmark instead of accepting detection.
    #[getter]
    fn manual_override(&self) -> bool {
        self.chain.provenance.placed_by_hand_at_sample.is_some()
    }

    /// The zero-based sample the reader placed, and None where the rule placed it.
    #[getter]
    fn placed_by_hand_at_sample(&self) -> Option<usize> {
        self.chain.provenance.placed_by_hand_at_sample
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
            "Provenance(method_id='{}', bound_parameters={}, registry_version={}, registry_declared_version={}, registry_digest={}, acquisition_complete={})",
            self.chain.provenance.method_id,
            format_parameters(&self.chain.provenance.bound_parameters()),
            optional(self.chain.provenance.registry_version.as_deref()),
            optional(self.chain.provenance.registry_declared_version.as_deref()),
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
    /// Samples matching the declared convention, counted apart from the samples that carry
    /// no number at all. None where the two were never counted separately.
    pub(crate) matched_the_convention: Option<usize>,
    pub(crate) carried_no_number: Option<usize>,
}

#[pymethods]
impl Exclusions {
    /// Every sample this step reported, which is the two counts below added together.
    #[getter]
    fn dropped_samples(&self) -> usize {
        self.inner.dropped_samples
    }

    /// Samples reading the value the declared convention writes for a measurement that was
    /// not taken.
    ///
    /// A plate with nothing on it reads zero or one quantisation step, and a vendor writing
    /// 0.00 to mean "no measurement" writes the same bytes, so on a jump trace the zero
    /// convention matches the whole flight phase: 157 samples of one real trial, every one
    /// of them a correct reading.
    #[getter]
    fn samples_matching_the_convention(&self) -> Option<usize> {
        self.matched_the_convention
    }

    /// Samples carrying no number, which is a gap in the recording rather than a convention
    /// a reader could have declared differently.
    #[getter]
    fn samples_carrying_no_number(&self) -> Option<usize> {
        self.carried_no_number
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
            "Exclusions(dropped_samples={}, samples_matching_the_convention={}, samples_carrying_no_number={}, sentinel_convention={}, reason={})",
            self.inner.dropped_samples,
            counted(self.matched_the_convention),
            counted(self.carried_no_number),
            optional(self.inner.sentinel_convention.as_deref()),
            optional(self.inner.reason.as_deref())
        )
    }
}

fn counted(value: Option<usize>) -> String {
    match value {
        Some(count) => count.to_string(),
        None => "None".to_string(),
    }
}

fn optional(value: Option<&str>) -> String {
    match value {
        Some(text) => format!("'{text}'"),
        None => "None".to_string(),
    }
}
