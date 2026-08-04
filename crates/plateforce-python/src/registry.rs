//! The registry as a Python object: what is on offer, what each entry claims, and what
//! binding a method to a set of parameter values commits you to.

use std::collections::BTreeMap;
use std::sync::Arc;

use plateforce_registry::{
    assemble, AssemblyError, Bias as CoreBias, Citation as CoreCitation,
    Construct as CoreConstruct, Disagreement as CoreDisagreement, Failure as CoreFailure,
    Gui as CoreGui, Method as CoreMethod, Parameter as CoreParameter, Preset as CorePreset,
    Registry as CoreRegistry, RegistryError as CoreRegistryError,
};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyType};

use crate::analysis::implemented_method_ids;
use crate::errors::{map_registry_error, parameter_error, MethodError, MethodNotImplementedError};

// Written by build.rs. The registry travels inside the extension module, so an install
// from PyPI reaches the methods without a clone of the repository beside it.
include!(concat!(env!("OUT_DIR"), "/embedded_registry.rs"));

/// The registry this wheel carries, assembled through the call the directory loader makes.
///
/// Strict in the same places: a set of files that loader refuses is refused here.
fn registry_this_build_carries() -> Result<CoreRegistry, CoreRegistryError> {
    let assembled =
        assemble(EMBEDDED_REGISTRY_FILES.iter().copied()).map_err(|error| match error {
            AssemblyError::Parse { path, source } => CoreRegistryError::Parse {
                path: path.into(),
                source,
            },
            AssemblyError::Unplaced { path } => CoreRegistryError::Unplaced { path: path.into() },
            AssemblyError::NoMethods => CoreRegistryError::Absent {
                path: std::path::PathBuf::from("the registry compiled into plateforce"),
                reason: "it holds no methods".to_string(),
            },
            AssemblyError::Duplicated(violations) => CoreRegistryError::Invalid(violations),
        })?;
    if !assembled.violations.is_empty() {
        return Err(CoreRegistryError::Invalid(assembled.violations));
    }
    let mut registry = assembled.registry;
    // The walk filters on the toml extension, so the revision the registry names itself is
    // not among the files and build.rs carries it separately.
    registry.declared_version = EMBEDDED_REGISTRY_VERSION.map(str::to_string);
    Ok(registry)
}

/// Which registry a result came from: the digest of the files that were read, and the
/// revision the caller pinned when they pinned one.
///
/// The registry is data and changes without a release, so a version nobody set is not
/// invented here. An unpinned result carries no version and is named by its digest, which
/// is measured from the bytes rather than asserted about them.
#[derive(Clone)]
pub struct RegistryIdentity {
    /// What every record this registry produces says about which registry produced it: the
    /// caller's pin, the registry's own claim, and the measured digest.
    pub stamp: plateforce_core::provenance::RegistryStamp,
    /// Every id this registry carries. A result reports a method as registry backed only
    /// when the registry both holds it and passed its own validator, and the rules a
    /// binding composes onto the one the caller named have to be judged the same way.
    ///
    /// Beside the stamp rather than in it: this is what the registry holds, and the stamp is
    /// what a result says about where it came from.
    pub method_ids: Arc<Vec<String>>,
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
    /// Published pipelines, counted as their own population. The four are never summed:
    /// a construct and a rule that fills it are not two of anything.
    #[pyo3(get)]
    preset_entries: usize,
}

#[pymethods]
impl Census {
    fn __repr__(&self) -> String {
        format!(
            "Census(constructs={}, computation_entries={}, protocol_entries={}, preset_entries={})",
            self.constructs, self.computation_entries, self.protocol_entries, self.preset_entries
        )
    }
}

/// A published pipeline: which rule fills each construct its source states, and the values
/// that source states for them.
///
/// Handed to `analyse_countermovement_jump` as `preset=`, which fills the slots it binds and
/// leaves the rest to the caller. Every value it supplies is recorded as cited, naming this
/// pipeline, so a result reached this way is a different record from one reached by typing
/// the same numbers.
#[pyclass(frozen, skip_from_py_object, module = "plateforce", name = "Preset")]
#[derive(Clone)]
pub struct Preset {
    pub(crate) inner: CorePreset,
    pub(crate) registry_identity: RegistryIdentity,
}

#[pymethods]
impl Preset {
    #[getter]
    fn id(&self) -> &str {
        &self.inner.id
    }

    #[getter]
    fn title(&self) -> &str {
        &self.inner.title
    }

    #[getter]
    fn description(&self) -> &str {
        &self.inner.description
    }

    /// The constructs this pipeline binds, and the rule it binds to each.
    #[getter]
    fn bindings(&self) -> BTreeMap<String, String> {
        self.inner
            .bindings
            .iter()
            .map(|binding| (binding.construct.clone(), binding.method_id.clone()))
            .collect()
    }

    /// The values this pipeline states, keyed by construct then by name.
    #[getter]
    fn parameters(&self) -> BTreeMap<String, BTreeMap<String, f64>> {
        self.inner
            .bindings
            .iter()
            .map(|binding| (binding.construct.clone(), binding.parameters.clone()))
            .collect()
    }

    /// Constructs this pipeline's source says nothing about. A fact about the source, so the
    /// caller decides those for themselves and the pipeline is not credited with the choice.
    #[getter]
    fn states_nothing_about(&self) -> Vec<String> {
        self.inner.states_nothing_about.clone()
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

    /// False when any source behind this pipeline rests on an abstract or a secondary
    /// source. A pipeline is only as citable as the weakest source behind it.
    #[getter]
    fn every_source_obtained(&self) -> bool {
        self.inner.every_source_obtained()
    }

    fn __repr__(&self) -> String {
        format!(
            "Preset('{}', binds={:?})",
            self.inner.id,
            self.bindings().keys().collect::<Vec<_>>()
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

/// One value a caller stated, in whichever of the two shapes a parameter varies by.
///
/// A parameter varies by number or by name and never both, so a request arriving in the other
/// shape is answered rather than coerced. Read as a number alone, `takeoff.op.crossing_selection`
/// accepted `selection = 1` on a parameter whose values are `first` and `longest_run`, and
/// nothing downstream could see it: an enumeration publishes no `published_values`, so the
/// check that catches an off-list number never looked.
#[derive(Debug, Clone)]
pub(crate) enum Stated {
    Number(f64),
    Name(String),
}

/// One rule of the registry this build carries, bound to the values stated and to the entry's
/// own default for every name that was not.
///
/// The route a notebook takes reaches its values through a `PyDict` and hands back a
/// `BoundMethod` whose record of which names the registry filled is module-private, so a guard
/// over the claim a request makes about a value nobody stated needs this to be written at all.
#[cfg(test)]
pub(crate) fn bound_from_the_registry_this_build_carries(
    method_id: &str,
    stated: BTreeMap<String, Stated>,
) -> BoundMethod {
    let carried = registry_this_build_carries().expect("the wheel carries a registry");
    let registry = Registry {
        inner: carried,
        version: None,
    };
    let identity = registry.identity();
    let inner = registry
        .inner
        .methods
        .get(method_id)
        .unwrap_or_else(|| panic!("{method_id} is not in the registry this build carries"))
        .clone();
    MethodEntry {
        inner,
        registry_identity: identity,
    }
    .binding_over(stated)
    .unwrap_or_else(|error| panic!("{method_id} did not bind: {error}"))
}

impl MethodEntry {
    /// The binding itself, over values already read out of whatever the caller handed in.
    ///
    /// Apart from `bind` because that call reaches its values through a `PyDict` and this one
    /// does not, so a guard over what a request claims about a value nobody stated can run
    /// without an interpreter.
    pub(crate) fn binding_over(&self, supplied: BTreeMap<String, Stated>) -> PyResult<BoundMethod> {
        let known: BTreeMap<&str, &CoreParameter> = self
            .inner
            .parameters
            .iter()
            .map(|parameter| (parameter.name.as_str(), parameter))
            .collect();

        // Sorted, so two bindings of the same values fingerprint the same however the
        // caller happened to order the keyword arguments.
        let mut bound: Vec<(String, f64)> = Vec::new();
        let mut chosen: Vec<(String, String)> = Vec::new();
        let mut defaulted: Vec<String> = Vec::new();
        let mut unpublished: Vec<String> = Vec::new();

        for (name, definition) in &known {
            let varies_by_name = !definition.named_values.is_empty();
            let value = match supplied.get(*name) {
                Some(given) => given.clone(),
                None => match (definition.default, &definition.default_key) {
                    (Some(default), _) => {
                        defaulted.push((*name).to_string());
                        Stated::Number(default)
                    }
                    (None, Some(key)) => {
                        defaulted.push((*name).to_string());
                        Stated::Name(key.clone())
                    }
                    (None, None) => {
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

            match value {
                Stated::Number(number) if varies_by_name => {
                    return Err(self.takes_a_name(name, definition, &number.to_string()))
                }
                Stated::Number(number) => {
                    if !definition.published_values.is_empty()
                        && !definition.published_values.contains(&number)
                    {
                        unpublished.push((*name).to_string());
                    }
                    bound.push(((*name).to_string(), number));
                }
                Stated::Name(key) if !varies_by_name => {
                    return Err(parameter_error(
                        &self.inner.id,
                        name,
                        format!(
                            "{}({}) is a number, and '{}' is a name",
                            self.inner.id, name, key
                        ),
                    ))
                }
                Stated::Name(key) => {
                    if !definition.named_values.iter().any(|value| value.key == key) {
                        return Err(self.takes_a_name(name, definition, &key));
                    }
                    chosen.push(((*name).to_string(), key));
                }
            }
        }

        Ok(BoundMethod {
            entry: self.clone(),
            bound_parameters: bound,
            bound_names: chosen,
            defaulted,
            unpublished,
        })
    }

    /// A value that is not one of the names this parameter takes, answered with the names it
    /// does take. The offered list is the whole point: a caller who reached here guessed, and
    /// a refusal that does not say what the entry accepts sends them to the registry file.
    fn takes_a_name(&self, name: &str, definition: &CoreParameter, given: &str) -> PyErr {
        let offered: Vec<&str> = definition
            .named_values
            .iter()
            .map(|value| value.key.as_str())
            .collect();
        parameter_error(
            &self.inner.id,
            name,
            format!(
                "{}({}) takes one of {:?}, got '{}'",
                self.inner.id, name, offered, given
            ),
        )
    }
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

    /// Whether this build can run the entry.
    #[getter]
    fn implemented(&self) -> bool {
        implemented_method_ids().contains(&self.inner.id.as_str())
    }

    /// Fix this method's parameter values, checking each against the entry.
    ///
    /// A parameter with a registry default may be omitted and the default is recorded as
    /// bound, whether the entry states that default as a number or as one of the names it
    /// publishes. A required parameter with no default of either shape has to be supplied. A
    /// value outside the entry's `published_values` binds and is listed in
    /// `unpublished_parameters`; a name the entry does not publish is refused, because an
    /// enumeration has no continuum for an unlisted value to sit on.
    #[pyo3(signature = (**parameters))]
    fn bind(&self, parameters: Option<&Bound<'_, PyDict>>) -> PyResult<BoundMethod> {
        let known: BTreeMap<&str, &CoreParameter> = self
            .inner
            .parameters
            .iter()
            .map(|parameter| (parameter.name.as_str(), parameter))
            .collect();

        let mut supplied: BTreeMap<String, Stated> = BTreeMap::new();
        if let Some(given) = parameters {
            for (key, value) in given.iter() {
                let name: String = key.extract()?;
                let Some(definition) = known.get(name.as_str()) else {
                    let offered: Vec<&str> = known.keys().copied().collect();
                    return Err(parameter_error(
                        &self.inner.id,
                        &name,
                        format!(
                            "{}: no parameter named '{}'. This entry takes {:?}",
                            self.inner.id, name, offered
                        ),
                    ));
                };
                // The entry decides which shape to read the argument in, so a string reaching
                // a numeric parameter and a number reaching an enumeration are each answered
                // by the parameter they arrived at rather than by whichever extract ran first.
                if !definition.named_values.is_empty() {
                    let chosen: String = value
                        .extract()
                        .map_err(|_| self.takes_a_name(&name, definition, &value.to_string()))?;
                    supplied.insert(name, Stated::Name(chosen));
                    continue;
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
                supplied.insert(name, Stated::Number(number));
            }
        }

        self.binding_over(supplied)
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
    /// The names bound for parameters the registry varies by name rather than by number.
    /// Beside the numbers rather than among them, because the engine reads the two through
    /// separate maps and a name flattened into a number is the option nobody can spell.
    pub(crate) bound_names: Vec<(String, String)>,
    defaulted: Vec<String>,
    unpublished: Vec<String>,
}

impl BoundMethod {
    pub(crate) fn registry_identity(&self) -> &RegistryIdentity {
        &self.entry.registry_identity
    }

    /// The names `bind` filled from the entry's own default because the caller named none.
    ///
    /// A binding carries these in `bound_parameters` beside the caller's own values, and the
    /// two are indistinguishable there. The request has a field for the difference and it can
    /// only be filled from here.
    pub(crate) fn names_the_registry_filled(&self) -> &[String] {
        &self.defaulted
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

    /// Every value this binding fixed, numbers as numbers and named options as their key.
    ///
    /// One mapping rather than two, because a reader asking what a rule bound is asking one
    /// question. Which shape a name comes back in is the entry's answer, not this call's.
    #[getter]
    fn parameters<'py>(&self, python: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let bound = PyDict::new(python);
        for (name, value) in &self.bound_parameters {
            bound.set_item(name, value)?;
        }
        for (name, key) in &self.bound_names {
            bound.set_item(name, key)?;
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
            .chain(
                self.bound_names
                    .iter()
                    .map(|(name, key)| format!("{name}={key}")),
            )
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
            stamp: plateforce_core::provenance::RegistryStamp::unpinned(
                self.inner.declared_version.clone(),
                Some(self.inner.content_digest.clone()),
            )
            .pinned_to(self.version.clone()),
            method_ids: Arc::new(self.inner.methods.keys().cloned().collect()),
        }
    }
}

#[pymethods]
impl Registry {
    /// `path` names a registry directory to read. Naming none reads the registry this
    /// build carries, which is the same set of files on every machine that installed the
    /// same wheel, so the digest a result reports is a property of the release rather than
    /// of the directory the caller happened to be sitting in.
    ///
    /// A named directory is read and never quietly replaced by the compiled-in copy.
    ///
    /// `version` pins which revision of the registry data produced a result, and a caller
    /// who pins nothing gets no version rather than a word standing in for one. Either
    /// way the result carries `digest`, taken from the files this call read.
    #[classmethod]
    #[pyo3(signature = (path = None, version = None))]
    fn load(
        _class: &Bound<'_, PyType>,
        path: Option<std::path::PathBuf>,
        version: Option<String>,
    ) -> PyResult<Self> {
        let inner = match path {
            Some(directory) => CoreRegistry::load(&directory).map_err(map_registry_error)?,
            None => registry_this_build_carries().map_err(map_registry_error)?,
        };
        Ok(Self { inner, version })
    }

    /// The revision this registry was pinned to, or None when nothing was pinned.
    ///
    /// A caller's declaration rather than the registry's, which is why it is separate from
    /// the one below.
    #[getter]
    fn version(&self) -> Option<&str> {
        self.version.as_deref()
    }

    /// The revision the registry names about itself, from the `VERSION` file beside its
    /// rules, or None where it names none.
    ///
    /// Distinct from `version` above, which is what a caller pinned. The two answer
    /// different questions and either can be present without the other: a caller can pin a
    /// revision a registry does not claim, and a registry can claim one nobody pinned.
    #[getter]
    fn declared_version(&self) -> Option<&str> {
        self.inner.declared_version.as_deref()
    }

    /// Identifies the files that were loaded. Two registries differing by one edited rule
    /// differ here, which is what a declared version cannot promise.
    #[getter]
    fn digest(&self) -> &str {
        &self.inner.content_digest
    }

    #[getter]
    fn census(&self) -> Census {
        // Destructured without a rest pattern, so a population added upstream is a compile
        // error here rather than a row that quietly stops being reported.
        let plateforce_registry::Census {
            constructs,
            computation_entries,
            protocol_entries,
            preset_entries,
        } = self.inner.census();
        Census {
            constructs,
            computation_entries,
            protocol_entries,
            preset_entries,
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

    /// Every published pipeline this registry carries.
    fn presets(&self) -> Vec<Preset> {
        self.inner
            .presets
            .values()
            .map(|inner| Preset {
                inner: inner.clone(),
                registry_identity: self.identity(),
            })
            .collect()
    }

    /// The pipeline a caller named, refused by name with the ones this registry carries.
    ///
    /// The sentence comes from the one place that writes it, so a name that is not a
    /// pipeline reads the same here as in a terminal, a browser tab and an R condition.
    fn preset(&self, preset_id: &str) -> PyResult<Preset> {
        match plateforce_analysis::request::preset_named(&self.inner, preset_id) {
            Ok(inner) => Ok(Preset {
                inner: inner.clone(),
                registry_identity: self.identity(),
            }),
            Err(refusal) => Err(MethodNotImplementedError::new_err(
                refusal.message().to_string(),
            )),
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
            "Registry(version={}, digest='{}', constructs={}, computation_entries={}, protocol_entries={}, preset_entries={})",
            optional(self.version.as_deref()),
            self.inner.content_digest,
            census.constructs,
            census.computation_entries,
            census.protocol_entries,
            census.preset_entries
        )
    }
}

fn optional(value: Option<&str>) -> String {
    match value {
        Some(text) => format!("'{text}'"),
        None => "None".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use plateforce_registry::{content_digest, read_sources, Registry as CoreRegistry, Source};

    /// The digest a wheel reports names the bytes the wheel carries.
    ///
    /// This is what makes a fingerprint in somebody's methods section checkable by a
    /// stranger: they install the version it names and compare.
    #[test]
    fn the_registry_in_the_wheel_is_the_registry_in_the_repository() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../registry");
        let on_disk = read_sources(&root).unwrap();

        let embedded_paths: Vec<&str> = super::EMBEDDED_REGISTRY_FILES
            .iter()
            .map(|(path, _)| *path)
            .collect();
        let disk_paths: Vec<&str> = on_disk.iter().map(|source| source.path.as_str()).collect();
        assert_eq!(disk_paths, embedded_paths);

        let carried = super::registry_this_build_carries().unwrap();
        assert_eq!(
            carried.content_digest,
            content_digest(on_disk.iter().map(Source::pair))
        );
    }

    /// A wheel that carries no methods refuses rather than handing back an empty registry
    /// that every later call reports as valid.
    #[test]
    fn the_wheel_carries_methods() {
        let carried = super::registry_this_build_carries().unwrap();
        assert!(!carried.methods.is_empty());
        assert!(!carried.constructs.is_empty());
    }

    /// The revision the registry names itself travels too, because the walk that collects
    /// entries filters on the toml extension and would leave a wheel unable to say which
    /// revision it holds.
    #[test]
    fn the_wheel_names_the_revision_the_registry_declares() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../registry");
        let on_disk = CoreRegistry::declared_version_at(&root);
        assert_eq!(
            super::registry_this_build_carries()
                .unwrap()
                .declared_version,
            on_disk
        );
        assert!(on_disk.is_some());
    }
}
