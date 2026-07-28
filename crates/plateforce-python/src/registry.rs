//! The registry as a Python object: what is on offer, what each entry claims, and what
//! binding a method to a set of parameter values commits you to.

use std::collections::BTreeMap;

use plateforce_registry::{
    Bias as CoreBias, Citation as CoreCitation, Construct as CoreConstruct,
    Disagreement as CoreDisagreement, Failure as CoreFailure, Gui as CoreGui, Method as CoreMethod,
    Parameter as CoreParameter, Registry as CoreRegistry,
};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyType};

use crate::analysis::implemented_method_ids;
use crate::errors::{map_registry_error, parameter_error, MethodError};

/// Which registry a result came from: the digest of the files that were read, and the
/// revision the caller pinned when they pinned one.
///
/// The registry is data and changes without a release, so a version nobody set is not
/// invented here. An unpinned result carries no version and is named by its digest, which
/// is measured from the bytes rather than asserted about them.
#[derive(Clone)]
pub struct RegistryIdentity {
    pub digest: Option<String>,
    pub version: Option<String>,
}

#[pyclass(frozen, skip_from_py_object, module = "plateforce", name = "Construct")]
#[derive(Clone)]
pub struct Construct {
    inner: CoreConstruct,
}

#[pymethods]
impl Construct {
    #[getter]
    fn id(&self) -> &str {
        &self.inner.id
    }

    #[getter]
    fn title(&self) -> &str {
        &self.inner.title
    }

    #[getter]
    fn unit(&self) -> &str {
        &self.inner.unit
    }

    #[getter]
    fn frame(&self) -> Option<&str> {
        self.inner.frame.as_deref()
    }

    #[getter]
    fn notes(&self) -> Option<&str> {
        self.inner.notes.as_deref()
    }

    fn __repr__(&self) -> String {
        format!(
            "Construct('{}', unit='{}', frame={})",
            self.inner.id,
            self.inner.unit,
            optional(self.inner.frame.as_deref())
        )
    }
}

#[pyclass(frozen, skip_from_py_object, module = "plateforce", name = "Parameter")]
#[derive(Clone)]
pub struct Parameter {
    inner: CoreParameter,
}

#[pymethods]
impl Parameter {
    #[getter]
    fn name(&self) -> &str {
        &self.inner.name
    }

    #[getter]
    fn unit(&self) -> Option<&str> {
        self.inner.unit.as_deref()
    }

    /// Every value the literature contains for this parameter.
    #[getter]
    fn published_values(&self) -> Vec<f64> {
        self.inner.published_values.clone()
    }

    #[getter]
    fn default(&self) -> Option<f64> {
        self.inner.default
    }

    #[getter]
    fn default_source(&self) -> Option<&str> {
        self.inner.default_source.as_deref()
    }

    #[getter]
    fn required(&self) -> bool {
        self.inner.required
    }

    #[getter]
    fn notes(&self) -> Option<&str> {
        self.inner.notes.as_deref()
    }

    fn __repr__(&self) -> String {
        format!(
            "Parameter('{}', unit={}, default={}, published_values={:?})",
            self.inner.name,
            optional(self.inner.unit.as_deref()),
            self.inner
                .default
                .map(|d| d.to_string())
                .unwrap_or_else(|| "None".to_string()),
            self.inner.published_values
        )
    }
}

#[pyclass(frozen, skip_from_py_object, module = "plateforce", name = "Citation")]
#[derive(Clone)]
pub struct Citation {
    inner: CoreCitation,
}

#[pymethods]
impl Citation {
    #[getter]
    fn key(&self) -> &str {
        &self.inner.key
    }

    /// proposes, uses, evaluates or disputes. A tool implementing a published method is
    /// an implementation, not a variant, and this is the field that says so.
    #[getter]
    fn role(&self) -> &'static str {
        use plateforce_registry::CitationRole::*;
        match self.inner.role {
            Proposes => "proposes",
            Uses => "uses",
            Evaluates => "evaluates",
            Disputes => "disputes",
        }
    }

    #[getter]
    fn reference(&self) -> &str {
        &self.inner.reference
    }

    #[getter]
    fn doi(&self) -> Option<&str> {
        self.inner.doi.as_deref()
    }

    /// False means the claim rests on an abstract or a secondary source.
    #[getter]
    fn obtained(&self) -> bool {
        self.inner.obtained
    }

    fn __repr__(&self) -> String {
        format!(
            "Citation('{}', role='{}', obtained={})",
            self.inner.key,
            self.role(),
            if self.inner.obtained { "True" } else { "False" }
        )
    }
}

#[pyclass(frozen, skip_from_py_object, module = "plateforce", name = "Bias")]
#[derive(Clone)]
pub struct Bias {
    inner: CoreBias,
}

#[pymethods]
impl Bias {
    #[getter]
    fn magnitude(&self) -> f64 {
        self.inner.magnitude
    }

    #[getter]
    fn unit(&self) -> &str {
        &self.inner.unit
    }

    #[getter]
    fn direction(&self) -> Option<&str> {
        self.inner.direction.as_deref()
    }

    /// What the bias was measured against. Two device-validation papers derive their
    /// reference plate's jump height from flight time, which makes their figures additive
    /// to flight-time bias rather than inclusive of it.
    #[getter]
    fn criterion(&self) -> &str {
        &self.inner.criterion
    }

    #[getter]
    fn criterion_kind(&self) -> &'static str {
        use plateforce_registry::CriterionKind::*;
        match self.inner.criterion_kind {
            HumanVisual => "human_visual",
            Instrument => "instrument",
            SimultaneousCapture => "simultaneous_capture",
            Model => "model",
        }
    }

    #[getter]
    fn source(&self) -> Option<&str> {
        self.inner.source.as_deref()
    }

    /// True when the figure describes only the trials on which the rule worked, in which
    /// case it has to be read next to the failure rate rather than on its own.
    #[getter]
    fn conditional_on_success(&self) -> bool {
        self.inner.conditional_on_success
    }

    fn __repr__(&self) -> String {
        format!(
            "Bias({} {} against '{}'{})",
            self.inner.magnitude,
            self.inner.unit,
            self.inner.criterion,
            if self.inner.conditional_on_success {
                ", conditional on the rule not failing"
            } else {
                ""
            }
        )
    }
}

/// A rule that can find the wrong event rather than merely find it late.
#[pyclass(frozen, skip_from_py_object, module = "plateforce", name = "Failure")]
#[derive(Clone)]
pub struct Failure {
    inner: CoreFailure,
}

#[pymethods]
impl Failure {
    #[getter]
    fn rate(&self) -> f64 {
        self.inner.rate
    }

    #[getter]
    fn numerator(&self) -> u32 {
        self.inner.numerator
    }

    #[getter]
    fn denominator(&self) -> u32 {
        self.inner.denominator
    }

    #[getter]
    fn corpus(&self) -> &str {
        &self.inner.corpus
    }

    #[getter]
    fn definition(&self) -> &str {
        &self.inner.definition
    }

    /// silent, loud or guarded. A rule returning an absurd value fails loudly if
    /// something is checking and invisibly if nothing is.
    #[getter]
    fn detectability(&self) -> &'static str {
        use plateforce_registry::Detectability::*;
        match self.inner.detectability {
            Silent => "silent",
            Loud => "loud",
            Guarded => "guarded",
        }
    }

    fn __repr__(&self) -> String {
        format!(
            "Failure({} of {} trials, {:.1}%, {}, on {})",
            self.inner.numerator,
            self.inner.denominator,
            self.inner.rate * 100.0,
            self.detectability(),
            self.inner.corpus
        )
    }
}

#[pyclass(
    frozen,
    skip_from_py_object,
    module = "plateforce",
    name = "Disagreement"
)]
#[derive(Clone)]
pub struct Disagreement {
    inner: CoreDisagreement,
}

#[pymethods]
impl Disagreement {
    #[getter]
    fn id(&self) -> &str {
        &self.inner.id
    }

    #[getter]
    fn kind(&self) -> &'static str {
        use plateforce_registry::DisagreementKind::*;
        match self.inner.kind {
            Genuine => "genuine",
            VendorConvention => "vendor_convention",
            Units => "units",
            Naming => "naming",
        }
    }

    #[getter]
    fn note(&self) -> Option<&str> {
        self.inner.note.as_deref()
    }

    fn __repr__(&self) -> String {
        format!("Disagreement('{}', kind='{}')", self.inner.id, self.kind())
    }
}

#[pyclass(frozen, skip_from_py_object, module = "plateforce", name = "Gui")]
#[derive(Clone)]
pub struct Gui {
    inner: CoreGui,
}

#[pymethods]
impl Gui {
    /// How hard an interface should push this choice at a user.
    #[getter]
    fn surfacing(&self) -> &'static str {
        use plateforce_registry::Surfacing::*;
        match self.inner.surfacing {
            DefaultAndHide => "default_and_hide",
            DefaultAndShow => "default_and_show",
            SurfaceOnDemand => "surface_on_demand",
            ForceADecision => "force_a_decision",
            NeverAUserChoice => "never_a_user_choice",
            Refuse => "refuse",
        }
    }

    #[getter]
    fn sensitivity(&self) -> Option<&str> {
        self.inner.sensitivity.as_deref()
    }

    #[getter]
    fn rationale(&self) -> Option<&str> {
        self.inner.rationale.as_deref()
    }

    fn __repr__(&self) -> String {
        format!(
            "Gui(surfacing='{}', sensitivity={})",
            self.surfacing(),
            optional(self.inner.sensitivity.as_deref())
        )
    }
}

/// Counts, each reported against the population it counts over. Computation and protocol
/// totals are never summed into one number.
#[pyclass(frozen, skip_from_py_object, module = "plateforce", name = "Census")]
#[derive(Clone)]
pub struct Census {
    #[pyo3(get)]
    constructs: usize,
    #[pyo3(get)]
    computation_entries: usize,
    #[pyo3(get)]
    protocol_entries: usize,
}

#[pymethods]
impl Census {
    fn __repr__(&self) -> String {
        format!(
            "Census(constructs={}, computation_entries={}, protocol_entries={})",
            self.constructs, self.computation_entries, self.protocol_entries
        )
    }
}

/// One registry entry: the rule, who proposed it, what it is biased against, and whether
/// it is known to find the wrong event.
#[pyclass(
    frozen,
    skip_from_py_object,
    module = "plateforce",
    name = "MethodEntry"
)]
#[derive(Clone)]
pub struct MethodEntry {
    pub(crate) inner: CoreMethod,
    pub(crate) registry_identity: RegistryIdentity,
}

#[pymethods]
impl MethodEntry {
    #[getter]
    fn id(&self) -> &str {
        &self.inner.id
    }

    #[getter]
    fn title(&self) -> &str {
        &self.inner.title
    }

    #[getter]
    fn construct(&self) -> &str {
        &self.inner.construct
    }

    #[getter]
    fn group(&self) -> Option<&str> {
        self.inner.group.as_deref()
    }

    /// The operational rule, as published.
    #[getter]
    fn rule(&self) -> &str {
        self.inner.rule.trim()
    }

    #[getter]
    fn status(&self) -> &'static str {
        use plateforce_registry::Status::*;
        match self.inner.status {
            Recommended => "recommended",
            Accepted => "accepted",
            Contested => "contested",
            Legacy => "legacy",
            Deprecated => "deprecated",
        }
    }

    #[getter]
    fn confidence(&self) -> &'static str {
        use plateforce_registry::Confidence::*;
        match self.inner.confidence {
            High => "high",
            Medium => "medium",
            Low => "low",
        }
    }

    /// Whether this entry is one side of a live argument, interoperability bookkeeping,
    /// or the only published rule.
    #[getter]
    fn debate(&self) -> Option<&'static str> {
        use plateforce_registry::Debate::*;
        self.inner.debate.map(|debate| match debate {
            Genuine => "genuine",
            VendorOrLegacy => "vendor_or_legacy",
            SinglePosition => "single_position",
        })
    }

    #[getter]
    fn parameters(&self) -> Vec<Parameter> {
        self.inner
            .parameters
            .iter()
            .map(|inner| Parameter {
                inner: inner.clone(),
            })
            .collect()
    }

    #[getter]
    fn citations(&self) -> Vec<Citation> {
        self.inner
            .citations
            .iter()
            .map(|inner| Citation {
                inner: inner.clone(),
            })
            .collect()
    }

    #[getter]
    fn biases(&self) -> Vec<Bias> {
        self.inner
            .biases
            .iter()
            .map(|inner| Bias {
                inner: inner.clone(),
            })
            .collect()
    }

    /// Present when the rule is known to find the wrong event on some proportion of
    /// trials. A bias figure for such a rule averages working with not working.
    #[getter]
    fn failure(&self) -> Option<Failure> {
        self.inner.failure.as_ref().map(|inner| Failure {
            inner: inner.clone(),
        })
    }

    #[getter]
    fn disagrees_with(&self) -> Vec<Disagreement> {
        self.inner
            .disagrees_with
            .iter()
            .map(|inner| Disagreement {
                inner: inner.clone(),
            })
            .collect()
    }

    #[getter]
    fn gui(&self) -> Option<Gui> {
        self.inner.gui.as_ref().map(|inner| Gui {
            inner: inner.clone(),
        })
    }

    /// Whether this build can run the entry. The registry describes the literature, which
    /// is larger than what any one piece of software implements.
    #[getter]
    fn implemented(&self) -> bool {
        implemented_method_ids().contains(&self.inner.id.as_str())
    }

    /// Fix this method's parameter values, checking each against the entry.
    ///
    /// A parameter with a registry default may be omitted and the default is recorded as
    /// bound. A required parameter with no default has to be supplied. A value outside the
    /// entry's `published_values` binds and is listed in `unpublished_parameters`.
    #[pyo3(signature = (**parameters))]
    fn bind(&self, parameters: Option<&Bound<'_, PyDict>>) -> PyResult<BoundMethod> {
        let known: BTreeMap<&str, &CoreParameter> = self
            .inner
            .parameters
            .iter()
            .map(|parameter| (parameter.name.as_str(), parameter))
            .collect();

        let mut supplied: BTreeMap<String, f64> = BTreeMap::new();
        if let Some(given) = parameters {
            for (key, value) in given.iter() {
                let name: String = key.extract()?;
                if !known.contains_key(name.as_str()) {
                    let offered: Vec<&str> = known.keys().copied().collect();
                    return Err(parameter_error(
                        &self.inner.id,
                        &name,
                        format!(
                            "{}: no parameter named '{}'. This entry takes {:?}",
                            self.inner.id, name, offered
                        ),
                    ));
                }
                let number: f64 = value.extract().map_err(|_| {
                    parameter_error(
                        &self.inner.id,
                        &name,
                        format!(
                            "{}({}) must be a number, got {}",
                            self.inner.id,
                            name,
                            value
                                .get_type()
                                .name()
                                .map(|n| n.to_string())
                                .unwrap_or_else(|_| "an unknown type".to_string())
                        ),
                    )
                })?;
                if !number.is_finite() {
                    return Err(parameter_error(
                        &self.inner.id,
                        &name,
                        format!(
                            "{}({} = {}) is not a finite number",
                            self.inner.id, name, number
                        ),
                    ));
                }
                supplied.insert(name, number);
            }
        }

        // Sorted, so two bindings of the same values fingerprint the same however the
        // caller happened to order the keyword arguments.
        let mut bound: Vec<(String, f64)> = Vec::new();
        let mut defaulted: Vec<String> = Vec::new();
        let mut unpublished: Vec<String> = Vec::new();

        for (name, definition) in &known {
            let value = match supplied.get(*name) {
                Some(given) => *given,
                None => match definition.default {
                    Some(default) => {
                        defaulted.push((*name).to_string());
                        default
                    }
                    None => {
                        if definition.required {
                            return Err(parameter_error(
                                &self.inner.id,
                                name,
                                format!(
                                    "{}: parameter '{}' is required and the registry gives it no default",
                                    self.inner.id, name
                                ),
                            ));
                        }
                        continue;
                    }
                },
            };
            if !definition.published_values.is_empty()
                && !definition.published_values.contains(&value)
            {
                unpublished.push((*name).to_string());
            }
            bound.push(((*name).to_string(), value));
        }

        Ok(BoundMethod {
            entry: self.clone(),
            bound_parameters: bound,
            defaulted,
            unpublished,
        })
    }

    fn __repr__(&self) -> String {
        let failure = match &self.inner.failure {
            Some(failure) => format!(
                ", FAILS on {} of {} trials ({:.1}%, {})",
                failure.numerator,
                failure.denominator,
                failure.rate * 100.0,
                match failure.detectability {
                    plateforce_registry::Detectability::Silent => "silent",
                    plateforce_registry::Detectability::Loud => "loud",
                    plateforce_registry::Detectability::Guarded => "guarded",
                }
            ),
            None => String::new(),
        };
        format!(
            "MethodEntry('{}', status='{}', implemented={}{})",
            self.inner.id,
            self.status(),
            if self.implemented() { "True" } else { "False" },
            failure
        )
    }
}

/// A method with its parameter values fixed. This is what an analysis takes, and what
/// ends up quoted in the provenance of every result it produces.
#[pyclass(
    frozen,
    skip_from_py_object,
    module = "plateforce",
    name = "BoundMethod"
)]
#[derive(Clone)]
pub struct BoundMethod {
    pub(crate) entry: MethodEntry,
    pub(crate) bound_parameters: Vec<(String, f64)>,
    defaulted: Vec<String>,
    unpublished: Vec<String>,
}

impl BoundMethod {
    pub(crate) fn registry_identity(&self) -> &RegistryIdentity {
        &self.entry.registry_identity
    }
}

#[pymethods]
impl BoundMethod {
    #[getter]
    pub(crate) fn method_id(&self) -> &str {
        &self.entry.inner.id
    }

    #[getter]
    fn entry(&self) -> MethodEntry {
        self.entry.clone()
    }

    #[getter]
    fn parameters<'py>(&self, python: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let bound = PyDict::new(python);
        for (name, value) in &self.bound_parameters {
            bound.set_item(name, value)?;
        }
        Ok(bound)
    }

    /// Parameters left to the registry default rather than chosen by the caller.
    #[getter]
    fn defaulted_parameters(&self) -> Vec<String> {
        self.defaulted.clone()
    }

    /// Parameters bound to a value the literature does not contain. Not an error, and
    /// worth knowing before the number is quoted next to a published one.
    #[getter]
    fn unpublished_parameters(&self) -> Vec<String> {
        self.unpublished.clone()
    }

    fn __repr__(&self) -> String {
        let body = self
            .bound_parameters
            .iter()
            .map(|(name, value)| format!("{name}={value}"))
            .collect::<Vec<_>>()
            .join(", ");
        format!("BoundMethod('{}', {})", self.entry.inner.id, body)
    }
}

/// A loaded registry. Loading is strict: an entry that breaks a schema rule stops the
/// whole file loading rather than producing a partial registry.
#[pyclass(frozen, skip_from_py_object, module = "plateforce", name = "Registry")]
pub struct Registry {
    inner: CoreRegistry,
    version: Option<String>,
}

impl Registry {
    /// What every entry handed out of this registry stamps on the results it produces.
    fn identity(&self) -> RegistryIdentity {
        RegistryIdentity {
            digest: Some(self.inner.content_digest.clone()),
            version: self.version.clone(),
        }
    }
}

#[pymethods]
impl Registry {
    /// `version` pins which revision of the registry data produced a result, and a caller
    /// who pins nothing gets no version rather than a word standing in for one. Either
    /// way the result carries `digest`, taken from the files this call read.
    #[classmethod]
    #[pyo3(signature = (path, version = None))]
    fn load(
        _class: &Bound<'_, PyType>,
        path: std::path::PathBuf,
        version: Option<String>,
    ) -> PyResult<Self> {
        let inner = CoreRegistry::load(&path).map_err(map_registry_error)?;
        Ok(Self { inner, version })
    }

    /// The revision this registry was pinned to, or None when nothing was pinned.
    #[getter]
    fn version(&self) -> Option<&str> {
        self.version.as_deref()
    }

    /// Identifies the files that were loaded. Two registries differing by one edited rule
    /// differ here, which is what a declared version cannot promise.
    #[getter]
    fn digest(&self) -> &str {
        &self.inner.content_digest
    }

    #[getter]
    fn census(&self) -> Census {
        let census = self.inner.census();
        Census {
            constructs: census.constructs,
            computation_entries: census.computation_entries,
            protocol_entries: census.protocol_entries,
        }
    }

    fn method_ids(&self) -> Vec<String> {
        self.inner.methods.keys().cloned().collect()
    }

    fn methods(&self) -> Vec<MethodEntry> {
        self.inner
            .methods
            .values()
            .map(|inner| MethodEntry {
                inner: inner.clone(),
                registry_identity: self.identity(),
            })
            .collect()
    }

    fn method(&self, method_id: &str) -> PyResult<MethodEntry> {
        match self.inner.methods.get(method_id) {
            Some(inner) => Ok(MethodEntry {
                inner: inner.clone(),
                registry_identity: self.identity(),
            }),
            None => Err(MethodError::new_err(format!(
                "no entry with id '{method_id}'. This registry holds {} computation entries",
                self.inner.methods.len()
            ))),
        }
    }

    fn constructs(&self) -> Vec<Construct> {
        self.inner
            .constructs
            .values()
            .map(|inner| Construct {
                inner: inner.clone(),
            })
            .collect()
    }

    /// Entries whose choice materially moves the number and on which the field is split.
    /// These are the rows an interface must not decide silently.
    fn genuine_debates(&self) -> Vec<MethodEntry> {
        self.inner
            .genuine_debates()
            .map(|inner| MethodEntry {
                inner: inner.clone(),
                registry_identity: self.identity(),
            })
            .collect()
    }

    /// Entries that can find the wrong event rather than merely find it late.
    fn methods_that_can_fail(&self) -> Vec<MethodEntry> {
        self.inner
            .methods_that_can_fail()
            .map(|inner| MethodEntry {
                inner: inner.clone(),
                registry_identity: self.identity(),
            })
            .collect()
    }

    /// Entries this build can run.
    fn implemented_methods(&self) -> Vec<MethodEntry> {
        self.methods()
            .into_iter()
            .filter(|entry| implemented_method_ids().contains(&entry.inner.id.as_str()))
            .collect()
    }

    fn __repr__(&self) -> String {
        let census = self.inner.census();
        format!(
            "Registry(version={}, digest='{}', constructs={}, computation_entries={}, protocol_entries={})",
            optional(self.version.as_deref()),
            self.inner.content_digest,
            census.constructs,
            census.computation_entries,
            census.protocol_entries
        )
    }
}

fn optional(value: Option<&str>) -> String {
    match value {
        Some(text) => format!("'{text}'"),
        None => "None".to_string(),
    }
}
