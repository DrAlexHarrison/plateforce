//! Running bound methods over a trial.
//!
//! Nothing here decides a method, resolves a parameter or computes a quantity.
//! `plateforce_analysis` does all three, for every surface. This file turns a
//! registry-bound method into the request that layer takes, and shapes what comes back
//! into the chain of choices a Python caller reads.

use std::collections::BTreeMap;

use plateforce_analysis::{
    bindings_for, AnalysisRequest, AnalysisResponse, BoundMethod as ResolvedMethod, MethodChoice,
    WeighingChoice, ONSET_OPERATOR_IDS,
};
use plateforce_core::{
    jump_height_from_flight_time as core_jump_height_from_flight_time, Measured as CoreMeasured,
    Provenance as CoreProvenance, STANDARD_GRAVITY_METERS_PER_SECOND_SQUARED,
};
use pyo3::prelude::*;

use crate::errors::{raise_refusal, MethodNotImplementedError, TrialError};
use crate::registry::{BoundMethod, Preset, RegistryIdentity};
use crate::result::{Exclusions, Measured, ProvenanceChain};
use crate::trial::Trial;

/// Steps the software performs that no registry entry describes, reported on every result
/// rather than left to be discovered.
// Five of these are registry entries and were being reported as unregistered on this surface
// alone. The rule was always the registry's; only the name this crate used for it was not, so
// the same number arrived carrying a resolvable id through the browser and an unresolvable one
// through Python. That is a parity break on the exact property the product exists to guarantee,
// and it is worse than either surface being uniformly wrong.
const TAKEOFF_VELOCITY_METHOD_ID: &str = "impulse.net_vertical.as_performance_determinant";
const NET_IMPULSE_METHOD_ID: &str = "impulse.net_vertical.as_performance_determinant";
const JUMP_HEIGHT_FROM_VELOCITY_METHOD_ID: &str = "jumpheight.takeoff.impulse_momentum";
const JUMP_HEIGHT_FROM_FLIGHT_TIME_METHOD_ID: &str = "jumpheight.takeoff.flight_time";
const RSI_MODIFIED_METHOD_ID: &str = "rsimod.jh_tov_over_ttt";

// Turning two landmark indices into an elapsed time carried no entry until the registry
// gained one for each. Both ids resolve, so neither is reported as unregistered.
const TIME_TO_TAKEOFF_METHOD_ID: &str = "time_to_takeoff.onset_to_takeoff";
const FLIGHT_TIME_METHOD_ID: &str = "flight_time.takeoff_to_touchdown";

const UNREGISTERED_METHOD_IDS: &[&str] = &[];

/// Registry entries this build can run, taken from the one list every surface reads. An
/// entry the registry describes and no rule implements has to fail rather than quietly
/// resolve to something near it.
pub fn implemented_method_ids() -> Vec<&'static str> {
    ["weighing", "onset", "takeoff"]
        .iter()
        .flat_map(|slot| bindings_for(slot))
        .map(|binding| binding.id)
        .collect()
}

/// The record comes from the one place that builds it, because this surface hand-wrote a
/// copy of the sentence and a copy is a second description of one failure.
fn expect_bound(python: Python<'_>, method: &BoundMethod, slot: &str) -> PyResult<()> {
    if bindings_for(slot).any(|binding| binding.id == method.method_id()) {
        return Ok(());
    }
    Err(raise_refusal(
        python,
        &plateforce_analysis::binding::unbound_method_refusal(method.method_id(), slot),
    ))
}

/// A construct with no rule behind it, and an id that is a real rule filed under a different
/// construct, are both refused here rather than reaching the engine. Either one alone would
/// match no binding, and a request that matches nothing comes back missing the number it
/// asked for with nothing said about it.
fn expect_derived_bound(
    python: Python<'_>,
    derived: &BTreeMap<String, BoundMethod>,
) -> PyResult<()> {
    let runs = plateforce_analysis::binding::derived_constructs();
    for (construct, method) in derived {
        if !runs.contains(&construct.as_str()) {
            return Err(raise_refusal(
                python,
                &plateforce_core::Refusal::construct_not_on_the_path(
                    construct.clone(),
                    runs.iter().map(|name| (*name).to_string()).collect(),
                ),
            ));
        }
        if !plateforce_analysis::binding::bindings_for_construct(construct)
            .any(|binding| binding.id == method.method_id())
        {
            return Err(raise_refusal(
                python,
                &plateforce_core::Refusal::method_not_implemented(
                    method.method_id(),
                    construct.clone(),
                    plateforce_analysis::binding::bindings_for_construct(construct)
                        .map(|binding| binding.id.to_string())
                        .collect(),
                ),
            ));
        }
    }
    Ok(())
}

/// The registry entry's own parameters, plus any the caller stated directly. A name no
/// rule reads is not dropped in silence: it comes back in `unread_parameters`.
fn quantities_of(
    method: &BoundMethod,
    stated: Option<BTreeMap<String, f64>>,
) -> BTreeMap<String, f64> {
    let mut parameters: BTreeMap<String, f64> = method.bound_parameters.iter().cloned().collect();
    parameters.extend(stated.unwrap_or_default());
    parameters
}

/// A choice for a slot the caller named a rule for, or one carrying only their values for a
/// slot a published pipeline is about to fill.
fn unbound_or(
    method: Option<&BoundMethod>,
    parameters: Option<BTreeMap<String, f64>>,
    options: Option<BTreeMap<String, String>>,
) -> MethodChoice {
    match method {
        Some(method) => choice_of(method, parameters, options),
        None => MethodChoice {
            parameters: parameters.unwrap_or_default(),
            options: options.unwrap_or_default(),
            ..Default::default()
        },
    }
}

fn choice_of(
    method: &BoundMethod,
    parameters: Option<BTreeMap<String, f64>>,
    options: Option<BTreeMap<String, String>>,
) -> MethodChoice {
    MethodChoice {
        method_id: method.method_id().to_string(),
        parameters: quantities_of(method, parameters),
        options: options.unwrap_or_default(),
        manual_index: None,
        ..Default::default()
    }
}

/// The provenance of one resolved rule, carrying what it read split the way the fingerprint
/// carries it: quantities against choices between named alternatives.
fn chain_of(
    resolved: &ResolvedMethod,
    registry: &RegistryIdentity,
    acquisition_complete: bool,
    depends_on: Vec<ProvenanceChain>,
) -> ProvenanceChain {
    let provenance = resolved.into_provenance(
        registry.version.clone(),
        registry.digest.clone(),
        acquisition_complete,
        depends_on
            .iter()
            .map(|input| input.provenance.clone())
            .collect(),
    );
    ProvenanceChain {
        enumerated_choices: provenance
            .choices
            .iter()
            .map(|choice| (choice.name.clone(), choice.value.clone()))
            .collect(),
        provenance,
        depends_on,
    }
}

/// A step the software performs over values it already computed, rather than a rule the
/// registry describes. Its inputs were measured from the trace, not supplied by a caller.
fn measured_records(
    pairs: Vec<(String, f64)>,
) -> Vec<plateforce_core::provenance::ParameterRecord> {
    use plateforce_core::provenance::{ParameterRecord, ParameterSource};
    pairs
        .into_iter()
        .map(|(name, value)| ParameterRecord {
            name,
            value,
            source: ParameterSource::Measured,
        })
        .collect()
}

fn software_step(
    method_id: &str,
    bound_parameters: Vec<(String, f64)>,
    registry: &RegistryIdentity,
    acquisition_complete: bool,
) -> CoreProvenance {
    CoreProvenance {
        parameters: measured_records(bound_parameters),
        registry_version: registry.version.clone(),
        registry_digest: registry.digest.clone(),
        acquisition_complete,
        ..CoreProvenance::of(method_id)
    }
}

fn resolved_slot<'a>(response: &'a AnalysisResponse, method_id: &str) -> &'a ResolvedMethod {
    response
        .bound_methods
        .iter()
        .find(|bound| bound.method_id == method_id)
        .expect("every method the request named is reported back against its id")
}

/// A landmark rule that placed nothing, raised as the error it was rather than as a
/// sentence, so a caller can branch on the parameter that failed.
fn refusal_of(python: Python<'_>, response: &AnalysisResponse, slot: &str) -> PyErr {
    let construct = plateforce_analysis::binding::construct_for_slot(slot).unwrap_or(slot);
    match response
        .refusals
        .iter()
        .find(|declined| declined.construct == construct)
    {
        // The record the rule built, with the id it was reached by and the construct it
        // filled stamped on, so the code and the sentence a Python caller sees are the ones
        // every other surface publishes. Both arms used to be told apart here, and the
        // second of them threw its code away.
        Some(declined) => raise_refusal(
            python,
            &plateforce_analysis::document::refusal_from_rule(declined),
        ),
        None => TrialError::new_err(format!(
            "the {construct} rule placed no landmark and gave no reason"
        )),
    }
}

struct Derived<'a> {
    response: &'a AnalysisResponse,
    registry: &'a RegistryIdentity,
    acquisition_complete: bool,
}

impl Derived<'_> {
    fn value(&self, key: &str) -> Option<f64> {
        self.response
            .metrics
            .iter()
            .find(|metric| metric.key == key)
            .and_then(|metric| metric.value)
    }

    /// From the quantity declaration rather than from the result, so a key that produced no
    /// value on this trial still reports the unit it would have been in.
    fn unit(&self, key: &str) -> &'static str {
        plateforce_analysis::response::quantity(key)
            .map(|declared| declared.unit)
            .unwrap_or_default()
    }

    /// A quantity the software derives from the landmarks, under an id of its own. Ten of
    /// the ids this package emits do not resolve in the registry, which is why every result
    /// lists them rather than presenting them as looked-up methods.
    fn measured(
        &self,
        key: &str,
        method_id: &str,
        bound_parameters: Vec<(String, f64)>,
        depends_on: Vec<ProvenanceChain>,
    ) -> Option<Measured> {
        self.value(key).map(|value| {
            Measured::new(
                CoreMeasured {
                    value,
                    unit: self.unit(key),
                    provenance: software_step(
                        method_id,
                        bound_parameters,
                        self.registry,
                        self.acquisition_complete,
                    ),
                },
                Vec::new(),
                depends_on,
            )
        })
    }

    /// Every quantity the response reported, keyed by the engine's own name for it, with
    /// the provenance of the rule that produced it.
    ///
    /// Read through `value()` rather than through a getter per quantity. Eleven getters
    /// were written when eleven quantities existed, and a rule bound for any other
    /// construct reports a key none of them names, so a transcription would go stale the
    /// first time one landed.
    fn every_value(&self) -> BTreeMap<String, Measured> {
        let mut values = BTreeMap::new();
        for metric in &self.response.metrics {
            let Some(value) = metric.value else { continue };
            let provenance = match &metric.computed_by {
                Some(id) => software_step(id, Vec::new(), self.registry, self.acquisition_complete),
                None => software_step("", Vec::new(), self.registry, self.acquisition_complete),
            };
            values.insert(
                metric.key.clone(),
                Measured::new(
                    CoreMeasured {
                        value,
                        unit: self.unit(&metric.key),
                        provenance,
                    },
                    Vec::new(),
                    Vec::new(),
                ),
            );
        }
        values
    }

    /// A quantity a resolved registry rule produced directly, so its provenance is that
    /// rule's rather than an id of the software's own.
    fn by_rule(&self, key: &str, chain: &ProvenanceChain) -> Option<Measured> {
        self.value(key).map(|value| {
            Measured::new(
                CoreMeasured {
                    value,
                    unit: self.unit(key),
                    provenance: chain.provenance.clone(),
                },
                chain.enumerated_choices.clone(),
                chain.depends_on.clone(),
            )
        })
    }
}

/// The results of one countermovement jump, each carrying the chain of choices behind it.
#[pyclass(
    frozen,
    skip_from_py_object,
    module = "plateforce",
    name = "CountermovementJump"
)]
pub struct CountermovementJump {
    system_weight_newtons: Measured,
    system_mass_kilograms: Measured,
    weighing_epoch_tied_window_count: usize,
    onset_index: usize,
    onset_time_seconds: Measured,
    takeoff_index: usize,
    takeoff_time_seconds: Measured,
    touchdown_index: Option<usize>,
    time_to_takeoff_seconds: Measured,
    flight_time_seconds: Option<Measured>,
    net_impulse_newton_seconds: Measured,
    takeoff_velocity_meters_per_second: Measured,
    jump_height_takeoff_frame_meters: Measured,
    jump_height_flight_time_meters: Option<Measured>,
    reactive_strength_index_modified: Option<Measured>,
    trial_exclusions: Exclusions,
    unregistered_methods: Vec<String>,
    unread_parameters: Vec<String>,
    assumed_parameters: Vec<String>,
    warnings: Vec<String>,
    /// Every quantity the engine reported, by its own name for it, reached through
    /// `value()`. The getters above cover the eleven the spine has always produced; a rule
    /// bound for any other construct reports through this and needs no getter of its own.
    values: BTreeMap<String, Measured>,
}

#[pymethods]
impl CountermovementJump {
    /// One quantity by the engine's name for it, matched in full.
    ///
    /// A name this analysis did not report is refused naming what it did, rather than
    /// answering `None`, because a caller reading a missing quantity as absent cannot tell
    /// it from a rule that ran and produced nothing.
    fn value(&self, quantity: &str) -> PyResult<Measured> {
        self.values.get(quantity).cloned().ok_or_else(|| {
            let refusal = plateforce_core::Refusal::unknown_parameter(
                "this analysis",
                quantity,
                self.values.keys().cloned().collect(),
            );
            Python::attach(|python| raise_refusal(python, &refusal))
        })
    }

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

    /// Where force returned above the threshold that placed takeoff. None when it never did.
    #[getter]
    fn touchdown_index(&self) -> Option<usize> {
        self.touchdown_index
    }

    /// The metric on which open implementations disagree most: two of them agree at
    /// r = 0.696 on this while agreeing at r = 0.961 on jump height.
    #[getter]
    fn time_to_takeoff_seconds(&self) -> Measured {
        self.time_to_takeoff_seconds.clone()
    }

    /// None when no touchdown was found, so no flight interval closes.
    #[getter]
    fn flight_time_seconds(&self) -> Option<Measured> {
        self.flight_time_seconds.clone()
    }

    #[getter]
    fn net_impulse_newton_seconds(&self) -> Measured {
        self.net_impulse_newton_seconds.clone()
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

    /// Height from flight time, a different construct from the takeoff-frame figure rather
    /// than a different way of computing it. None when no touchdown was found.
    #[getter]
    fn jump_height_flight_time_meters(&self) -> Option<Measured> {
        self.jump_height_flight_time_meters.clone()
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

    /// Names passed in that no rule read, so their values did not reach the answer.
    #[getter]
    fn unread_parameters(&self) -> Vec<String> {
        self.unread_parameters.clone()
    }

    /// Names in the provenance that nobody chose, so a rule used its own value.
    #[getter]
    fn assumed_parameters(&self) -> Vec<String> {
        self.assumed_parameters.clone()
    }

    /// What the rules reported about this trace while placing the landmarks.
    #[getter]
    fn warnings(&self) -> Vec<String> {
        self.warnings.clone()
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

/// The one place this surface writes an analysis request.
///
/// Every entry point that sends one goes through it: the shaped analysis, the engine
/// document below it, and the sweep, whose unvaried combination has to be the request a
/// user's own analysis call sends or the sweep is around a different result. A second
/// builder beside this one would make the cross-surface comparison a statement about the
/// second builder rather than about the product.
#[allow(clippy::too_many_arguments)]
pub(crate) fn analysis_request_of(
    python: Python<'_>,
    weighing_epoch: Option<&BoundMethod>,
    onset: Option<&BoundMethod>,
    takeoff: Option<&BoundMethod>,
    preset: Option<&Preset>,
    gravity_meters_per_second_squared: f64,
    weighing_parameters: Option<BTreeMap<String, f64>>,
    onset_parameters: Option<BTreeMap<String, f64>>,
    takeoff_parameters: Option<BTreeMap<String, f64>>,
    weighing_options: Option<BTreeMap<String, String>>,
    onset_options: Option<BTreeMap<String, String>>,
    takeoff_options: Option<BTreeMap<String, String>>,
    weighing_start_index: Option<usize>,
    onset_index: Option<usize>,
    takeoff_index: Option<usize>,
    touchdown_index: Option<usize>,
    derived: Option<BTreeMap<String, Py<BoundMethod>>>,
    derived_parameters: Option<BTreeMap<String, BTreeMap<String, f64>>>,
) -> PyResult<(AnalysisRequest, RegistryIdentity)> {
    // A pipeline fills the constructs its source states, so a caller who named one leaves
    // those arguments out. Whatever is still unnamed once it has been laid on is refused by
    // name below rather than resolved to a neighbouring rule.
    for (method, slot) in [
        (weighing_epoch, "weighing"),
        (onset, "onset"),
        (takeoff, "takeoff"),
    ] {
        if let Some(method) = method {
            expect_bound(python, method, slot)?;
        }
    }
    // A `BoundMethod` reaches a signature as the Python object holding it, so each is
    // borrowed once here and the request is built from plain values after that.
    let derived: BTreeMap<String, BoundMethod> = derived
        .unwrap_or_default()
        .into_iter()
        .map(|(construct, method)| (construct, method.borrow(python).clone()))
        .collect();
    let derived_parameters = derived_parameters.unwrap_or_default();
    expect_derived_bound(python, &derived)?;

    // Every rule this call holds carries the registry it came from, and a pipeline carries
    // one too, so the identity stamped on the record is the first of them that exists
    // rather than a field only one argument could supply.
    let registry = [
        weighing_epoch.map(|m| m.registry_identity()),
        onset.map(|m| m.registry_identity()),
        takeoff.map(|m| m.registry_identity()),
        preset.map(|p| &p.registry_identity),
    ]
    .into_iter()
    .flatten()
    .next()
    .ok_or_else(|| {
        MethodNotImplementedError::new_err(
            "no rule and no published pipeline was named for this analysis".to_string(),
        )
    })?
    .clone();

    let mut request = AnalysisRequest {
        weighing: match weighing_epoch {
            Some(method) => WeighingChoice {
                method_id: method.method_id().to_string(),
                start_index: weighing_start_index,
                parameters: quantities_of(method, weighing_parameters),
                options: weighing_options.unwrap_or_default(),
                ..Default::default()
            },
            None => WeighingChoice {
                start_index: weighing_start_index,
                parameters: weighing_parameters.unwrap_or_default(),
                options: weighing_options.unwrap_or_default(),
                ..Default::default()
            },
        },
        onset: MethodChoice {
            manual_index: onset_index,
            ..unbound_or(onset, onset_parameters, onset_options)
        },
        takeoff: MethodChoice {
            manual_index: takeoff_index,
            ..unbound_or(takeoff, takeoff_parameters, takeoff_options)
        },
        touchdown_index,
        gravity_meters_per_second_squared,
        // What this registry carries. The binding composes operators onto the rule the
        // caller named, and those are entries in their own right that have to be judged
        // against the same list rather than assumed.
        registry_backed_ids: registry.method_ids.as_ref().clone(),
        derived: derived
            .iter()
            .map(|(construct, method)| {
                (
                    construct.clone(),
                    choice_of(method, derived_parameters.get(construct).cloned(), None),
                )
            })
            .collect(),
        ..Default::default()
    };

    // Laid on after the caller's own values, so a value they stated keeps its place and the
    // pipeline's is recorded beside it as the one it displaced.
    if let Some(preset) = preset {
        request
            .adopt(&preset.inner)
            .map_err(|refusal| raise_refusal(python, &refusal))?;
    }

    Ok((request, registry))
}

/// The engine's own document for one analysis, in the envelope every surface answers in.
///
/// Which registry the numbers came from travels beside them. It is read off the rules and
/// the pipeline the call named rather than restated by the caller, so it cannot name a
/// registry the rules did not come out of.
#[derive(serde::Serialize)]
struct AnalysisDocument<'a> {
    #[serde(flatten)]
    response: &'a AnalysisResponse,
    registry_digest: Option<String>,
    registry_version: Option<String>,
    acquisition_complete: bool,
}

/// The engine's own record of one analysis, as the engine wrote it.
///
/// `analyse_countermovement_jump` reshapes that record into the classes a notebook reads and
/// keeps no copy of it, so nothing on this surface could be handed to a comparison against
/// another surface. This returns it whole, through the request builder that call uses. The
/// same primitive sits behind R's `pf_analyse` and the browser's `analyse`, and is private
/// here for the reason it is private there: the shaped answer is what a caller reads.
///
/// A rule that declines raises, carrying every field the record holds, because a caller
/// meeting a refusal here meets the exception `analyse_countermovement_jump` raises rather
/// than a second shape to parse.
#[pyfunction]
#[pyo3(name = "_analyse_json")]
#[pyo3(signature = (
    trial,
    weighing_epoch = None,
    onset = None,
    takeoff = None,
    preset = None,
    gravity_meters_per_second_squared = STANDARD_GRAVITY_METERS_PER_SECOND_SQUARED,
    weighing_parameters = None,
    onset_parameters = None,
    takeoff_parameters = None,
    weighing_options = None,
    onset_options = None,
    takeoff_options = None,
    weighing_start_index = None,
    onset_index = None,
    takeoff_index = None,
    touchdown_index = None,
    derived = None,
    derived_parameters = None,
))]
#[allow(clippy::too_many_arguments)]
pub fn analyse_json(
    python: Python<'_>,
    trial: &Trial,
    weighing_epoch: Option<&BoundMethod>,
    onset: Option<&BoundMethod>,
    takeoff: Option<&BoundMethod>,
    preset: Option<&Preset>,
    gravity_meters_per_second_squared: f64,
    weighing_parameters: Option<BTreeMap<String, f64>>,
    onset_parameters: Option<BTreeMap<String, f64>>,
    takeoff_parameters: Option<BTreeMap<String, f64>>,
    weighing_options: Option<BTreeMap<String, String>>,
    onset_options: Option<BTreeMap<String, String>>,
    takeoff_options: Option<BTreeMap<String, String>>,
    weighing_start_index: Option<usize>,
    onset_index: Option<usize>,
    takeoff_index: Option<usize>,
    touchdown_index: Option<usize>,
    derived: Option<BTreeMap<String, Py<BoundMethod>>>,
    derived_parameters: Option<BTreeMap<String, BTreeMap<String, f64>>>,
) -> PyResult<String> {
    let (request, registry) = analysis_request_of(
        python,
        weighing_epoch,
        onset,
        takeoff,
        preset,
        gravity_meters_per_second_squared,
        weighing_parameters,
        onset_parameters,
        takeoff_parameters,
        weighing_options,
        onset_options,
        takeoff_options,
        weighing_start_index,
        onset_index,
        takeoff_index,
        touchdown_index,
        derived,
        derived_parameters,
    )?;

    let response = plateforce_analysis::run(&trial.inner, &request)
        .map_err(|refusal| raise_refusal(python, &refusal))?;

    let document = AnalysisDocument {
        response: &response,
        registry_digest: registry.digest.clone(),
        registry_version: registry.version.clone(),
        acquisition_complete: trial.acquisition_complete(),
    };
    serde_json::to_string(&serde_json::json!({ "ok": document }))
        .map_err(|error| TrialError::new_err(error.to_string()))
}

/// Analyse one countermovement jump with the methods named.
///
/// The three method arguments are bound registry entries and appear in the provenance of
/// every result. Their numeric parameters ride on the binding, and `*_parameters` states
/// any the entry does not carry. The `*_options` arguments carry the choices the registry
/// states as enumerations rather than numbers, under the names the registry publishes for
/// them, which are the names the browser uses too.
///
/// Passing a name no rule reads is not silently dropped: it comes back in
/// `unread_parameters`, and a value nobody chose comes back in `assumed_parameters`.
#[pyfunction]
#[pyo3(signature = (
    trial,
    weighing_epoch = None,
    onset = None,
    takeoff = None,
    preset = None,
    gravity_meters_per_second_squared = STANDARD_GRAVITY_METERS_PER_SECOND_SQUARED,
    weighing_parameters = None,
    onset_parameters = None,
    takeoff_parameters = None,
    weighing_options = None,
    onset_options = None,
    takeoff_options = None,
    weighing_start_index = None,
    onset_index = None,
    takeoff_index = None,
    touchdown_index = None,
    derived = None,
    derived_parameters = None,
))]
#[allow(clippy::too_many_arguments)]
pub fn analyse_countermovement_jump(
    python: Python<'_>,
    trial: &Trial,
    weighing_epoch: Option<&BoundMethod>,
    onset: Option<&BoundMethod>,
    takeoff: Option<&BoundMethod>,
    preset: Option<&Preset>,
    gravity_meters_per_second_squared: f64,
    weighing_parameters: Option<BTreeMap<String, f64>>,
    onset_parameters: Option<BTreeMap<String, f64>>,
    takeoff_parameters: Option<BTreeMap<String, f64>>,
    weighing_options: Option<BTreeMap<String, String>>,
    onset_options: Option<BTreeMap<String, String>>,
    takeoff_options: Option<BTreeMap<String, String>>,
    weighing_start_index: Option<usize>,
    onset_index: Option<usize>,
    takeoff_index: Option<usize>,
    touchdown_index: Option<usize>,
    derived: Option<BTreeMap<String, Py<BoundMethod>>>,
    derived_parameters: Option<BTreeMap<String, BTreeMap<String, f64>>>,
) -> PyResult<CountermovementJump> {
    let (request, registry) = analysis_request_of(
        python,
        weighing_epoch,
        onset,
        takeoff,
        preset,
        gravity_meters_per_second_squared,
        weighing_parameters,
        onset_parameters,
        takeoff_parameters,
        weighing_options,
        onset_options,
        takeoff_options,
        weighing_start_index,
        onset_index,
        takeoff_index,
        touchdown_index,
        derived,
        derived_parameters,
    )?;
    let acquisition_complete = trial.acquisition_complete();

    // The record the engine built, raised under the class its own code names. This used to
    // arrive as a sentence, so every one of these was a `TrialError` whatever it was about.
    let response = plateforce_analysis::run(&trial.inner, &request)
        .map_err(|refusal| raise_refusal(python, &refusal))?;

    let onset_index = response
        .onset_index
        .ok_or_else(|| refusal_of(python, &response, "onset"))?;
    let takeoff_index = response
        .takeoff_index
        .ok_or_else(|| refusal_of(python, &response, "takeoff"))?;

    let epoch_chain = chain_of(
        resolved_slot(&response, &request.weighing.method_id),
        &registry,
        acquisition_complete,
        Vec::new(),
    );
    // An operator is a registry entry with its own citation and its own default, so it
    // stands in the chain beside the epoch the threshold rule rests on rather than folded
    // into that rule's parameters.
    let mut onset_inputs: Vec<ProvenanceChain> = response
        .bound_methods
        .iter()
        .filter(|bound| ONSET_OPERATOR_IDS.contains(&bound.method_id.as_str()))
        .map(|bound| chain_of(bound, &registry, acquisition_complete, Vec::new()))
        .collect();
    onset_inputs.push(epoch_chain.clone());
    let onset_chain = chain_of(
        resolved_slot(&response, &request.onset.method_id),
        &registry,
        acquisition_complete,
        onset_inputs,
    );
    let takeoff_chain = chain_of(
        resolved_slot(&response, &request.takeoff.method_id),
        &registry,
        acquisition_complete,
        vec![epoch_chain.clone()],
    );

    let derived = Derived {
        response: &response,
        registry: &registry,
        acquisition_complete,
    };
    let gravity_parameter = vec![(
        "gravity_meters_per_second_squared".to_string(),
        gravity_meters_per_second_squared,
    )];
    let interval = vec![onset_chain.clone(), takeoff_chain.clone()];
    let whole_pipeline = vec![
        epoch_chain.clone(),
        onset_chain.clone(),
        takeoff_chain.clone(),
    ];

    let time_to_takeoff = derived
        .measured(
            "time_to_takeoff_seconds",
            TIME_TO_TAKEOFF_METHOD_ID,
            Vec::new(),
            interval.clone(),
        )
        .ok_or_else(|| refusal_of(python, &response, "onset"))?;
    let net_impulse = derived
        .measured(
            "net_impulse_newton_seconds",
            NET_IMPULSE_METHOD_ID,
            Vec::new(),
            whole_pipeline.clone(),
        )
        .ok_or_else(|| refusal_of(python, &response, "takeoff"))?;
    let velocity = derived
        .measured(
            "takeoff_velocity_meters_per_second",
            TAKEOFF_VELOCITY_METHOD_ID,
            gravity_parameter.clone(),
            whole_pipeline,
        )
        .ok_or_else(|| refusal_of(python, &response, "takeoff"))?;
    let jump_height = derived
        .measured(
            "jump_height_from_takeoff_meters",
            JUMP_HEIGHT_FROM_VELOCITY_METHOD_ID,
            gravity_parameter.clone(),
            vec![velocity.chain()],
        )
        .ok_or_else(|| refusal_of(python, &response, "takeoff"))?;
    let flight_time = derived.measured(
        "flight_time_seconds",
        FLIGHT_TIME_METHOD_ID,
        Vec::new(),
        vec![takeoff_chain.clone()],
    );
    let jump_height_flight_time = derived.measured(
        "jump_height_from_flight_time_meters",
        JUMP_HEIGHT_FROM_FLIGHT_TIME_METHOD_ID,
        gravity_parameter.clone(),
        flight_time.iter().map(Measured::chain).collect(),
    );
    let rsi = derived.measured(
        "reactive_strength_index_modified",
        RSI_MODIFIED_METHOD_ID,
        Vec::new(),
        vec![jump_height.chain(), time_to_takeoff.chain()],
    );

    let system_weight = derived
        .by_rule("system_weight_newtons", &epoch_chain)
        .ok_or_else(|| refusal_of(python, &response, "weighing"))?;
    let system_mass = Measured::new(
        CoreMeasured {
            value: derived.value("system_mass_kilograms").unwrap_or_default(),
            unit: derived.unit("system_mass_kilograms"),
            provenance: CoreProvenance {
                parameters: measured_records(gravity_parameter),
                ..epoch_chain.provenance.clone()
            },
        },
        epoch_chain.enumerated_choices.clone(),
        Vec::new(),
    );

    Ok(CountermovementJump {
        system_weight_newtons: system_weight,
        system_mass_kilograms: system_mass,
        weighing_epoch_tied_window_count: response.weighing_epoch_tied_window_count,
        onset_index,
        onset_time_seconds: derived
            .by_rule("onset_time_seconds", &onset_chain)
            .ok_or_else(|| refusal_of(python, &response, "onset"))?,
        takeoff_index,
        takeoff_time_seconds: derived
            .by_rule("takeoff_time_seconds", &takeoff_chain)
            .ok_or_else(|| refusal_of(python, &response, "takeoff"))?,
        touchdown_index: response.touchdown_index,
        time_to_takeoff_seconds: time_to_takeoff,
        flight_time_seconds: flight_time,
        net_impulse_newton_seconds: net_impulse,
        takeoff_velocity_meters_per_second: velocity,
        jump_height_takeoff_frame_meters: jump_height,
        jump_height_flight_time_meters: jump_height_flight_time,
        reactive_strength_index_modified: rsi,
        trial_exclusions: trial.exclusions_for_result(),
        unregistered_methods: UNREGISTERED_METHOD_IDS
            .iter()
            .map(|id| (*id).to_string())
            .collect(),
        unread_parameters: response
            .bound_methods
            .iter()
            .flat_map(|bound| bound.unread_parameters.iter().cloned())
            .collect(),
        assumed_parameters: response
            .bound_methods
            .iter()
            .flat_map(|bound| bound.assumed_parameters())
            .collect(),
        warnings: response.warnings.clone(),
        values: derived.every_value(),
    })
}

/// Jump height from a flight time, in metres.
///
/// A different construct from the takeoff-frame height an analysis returns, not a
/// different way of computing the same one. Exposed on its own because nothing in the core
/// places landing, so a flight time has to come from elsewhere, such as a contact mat.
///
/// This route reads no registry, so the result carries no digest and takes whichever
/// revision the caller names.
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
    Measured::new(
        CoreMeasured {
            value: core_jump_height_from_flight_time(
                flight_time_seconds,
                gravity_meters_per_second_squared,
            ),
            unit: "meters",
            provenance: CoreProvenance {
                parameters: measured_records(vec![
                    ("flight_time_seconds".to_string(), flight_time_seconds),
                    (
                        "gravity_meters_per_second_squared".to_string(),
                        gravity_meters_per_second_squared,
                    ),
                ]),
                registry_version,
                acquisition_complete,
                ..CoreProvenance::of(JUMP_HEIGHT_FROM_FLIGHT_TIME_METHOD_ID)
            },
        },
        Vec::new(),
        Vec::new(),
    )
}

/// Takeoff by the shape of the rise out of each low-force run, and how many landings the
/// recording holds.
///
/// The rule that tells a landing apart from the reweighting into propulsion. Exposed here
/// because the research harness that ruled and measured it has to call this implementation
/// rather than keeping its own: two implementations of one quantity is the finding this
/// project exists to publish.
///
/// Returns the sample takeoff was placed on, or `None` when the recording closes no run with
/// a landing, and the number of landings found so a caller can say when there was more than
/// one rather than silently reporting the first.
#[pyfunction]
#[pyo3(signature = (
    vertical_force_newtons,
    system_weight_newtons,
    threshold_newtons,
    sample_rate_hz,
))]
pub fn takeoff_by_landing_shape(
    vertical_force_newtons: Vec<f64>,
    system_weight_newtons: f64,
    threshold_newtons: f64,
    sample_rate_hz: f64,
) -> (Option<usize>, usize) {
    plateforce_core::takeoff::landing_shape::takeoff_by_landing_shape(
        &vertical_force_newtons,
        system_weight_newtons,
        threshold_newtons,
        sample_rate_hz,
        &plateforce_core::takeoff::landing_shape::LandingShapeSpec::default(),
    )
}

/// Every low-force run in a trace, with the shape of the rise out of it and the verdict that
/// follows. The diagnostic view behind `takeoff_by_landing_shape`, exposed so the research
/// harness can report on the rule without reimplementing it.
#[pyfunction]
#[pyo3(signature = (
    vertical_force_newtons,
    system_weight_newtons,
    threshold_newtons,
    sample_rate_hz,
))]
pub fn classify_low_force_runs(
    python: Python<'_>,
    vertical_force_newtons: Vec<f64>,
    system_weight_newtons: f64,
    threshold_newtons: f64,
    sample_rate_hz: f64,
) -> PyResult<Vec<Py<PyAny>>> {
    use plateforce_core::takeoff::landing_shape::{classify_runs, LandingShapeSpec};
    classify_runs(
        &vertical_force_newtons,
        system_weight_newtons,
        threshold_newtons,
        sample_rate_hz,
        &LandingShapeSpec::default(),
    )
    .into_iter()
    .map(|run| {
        let entry = pyo3::types::PyDict::new(python);
        entry.set_item("start_sample", run.start_sample)?;
        entry.set_item("end_sample", run.end_sample)?;
        entry.set_item("duration_seconds", run.duration_seconds)?;
        entry.set_item("ends_the_recording", run.ends_the_recording)?;
        entry.set_item("is_flight", run.is_flight)?;
        entry.set_item(
            "shape",
            run.shape
                .map(|shape| shape_as_dict(python, shape))
                .transpose()?,
        )?;
        Ok(entry.into_any().unbind())
    })
    .collect()
}

fn shape_as_dict(
    python: Python<'_>,
    shape: plateforce_core::takeoff::landing_shape::RiseShape,
) -> PyResult<Py<PyAny>> {
    let entry = pyo3::types::PyDict::new(python);
    entry.set_item("rise_fullness", shape.rise_fullness)?;
    entry.set_item(
        "peak_rise_rate_bodyweights_per_second",
        shape.peak_rise_rate_bodyweights_per_second,
    )?;
    entry.set_item("peak_bodyweights", shape.peak_bodyweights)?;
    entry.set_item("rise_seconds", shape.rise_seconds)?;
    entry.set_item("peak_sample", shape.peak_sample)?;
    Ok(entry.into_any().unbind())
}

/// The shape numbers for the rise out of one run, or nothing when there is no rise to read.
#[pyfunction]
#[pyo3(signature = (
    vertical_force_newtons,
    run_end_sample,
    system_weight_newtons,
    sample_rate_hz,
))]
pub fn rise_after_run(
    python: Python<'_>,
    vertical_force_newtons: Vec<f64>,
    run_end_sample: usize,
    system_weight_newtons: f64,
    sample_rate_hz: f64,
) -> PyResult<Option<Py<PyAny>>> {
    use plateforce_core::takeoff::landing_shape::{rise_after, LandingShapeSpec};
    rise_after(
        &vertical_force_newtons,
        run_end_sample,
        system_weight_newtons,
        sample_rate_hz,
        &LandingShapeSpec::default(),
    )
    .map(|shape| shape_as_dict(python, shape))
    .transpose()
}

/// Whether a rise read by `rise_after_run` is a collision rather than a muscular push.
#[pyfunction]
pub fn rise_looks_like_a_landing(
    peak_rise_rate_bodyweights_per_second: f64,
    peak_bodyweights: f64,
) -> bool {
    let spec = plateforce_core::takeoff::landing_shape::LandingShapeSpec::default();
    peak_rise_rate_bodyweights_per_second >= spec.landing_rise_rate_floor_bodyweights_per_second
        && peak_bodyweights >= spec.landing_peak_floor_bodyweights
}
