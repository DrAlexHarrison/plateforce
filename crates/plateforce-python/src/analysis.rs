//! Running bound methods over a trial.
//!
//! Nothing here decides a method, resolves a parameter or computes a quantity.
//! `plateforce_analysis` does all three, for every surface. This file turns a
//! registry-bound method into the request that layer takes, and shapes what comes back
//! into the chain of choices a Python caller reads.

use std::collections::BTreeMap;

use plateforce_analysis::{
    bindings_for, AnalysisRequest, AnalysisResponse, BoundMethod as ResolvedMethod, MethodChoice,
    RuleRefusal, WeighingChoice, ONSET_OPERATOR_IDS,
};
use plateforce_core::{
    jump_height_from_flight_time as core_jump_height_from_flight_time, Measured as CoreMeasured,
    Provenance as CoreProvenance, STANDARD_GRAVITY_METERS_PER_SECOND_SQUARED,
};
use pyo3::prelude::*;

use crate::errors::{map_trial_error, MethodNotImplementedError, TrialError};
use crate::registry::{BoundMethod, RegistryIdentity};
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

fn expect_bound(method: &BoundMethod, slot: &str) -> PyResult<()> {
    let available: Vec<&str> = bindings_for(slot).map(|binding| binding.id).collect();
    if available.contains(&method.method_id()) {
        return Ok(());
    }
    Err(MethodNotImplementedError::new_err(format!(
        "'{}' was passed as the {slot} method, and the rules available for that step are {available:?}",
        method.method_id()
    )))
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
    ProvenanceChain {
        provenance: CoreProvenance {
            method_id: resolved.method_id.clone(),
            bound_parameters: resolved.quantities(),
            registry_version: registry.version.clone(),
            registry_digest: registry.digest.clone(),
            acquisition_complete,
        },
        enumerated_choices: resolved.enumerated_choices(),
        depends_on,
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
    match response
        .refusals
        .iter()
        .find(|(named, _)| *named == slot)
        .map(|(_, refusal)| refusal)
    {
        Some(RuleRefusal::Trial(error)) => map_trial_error(python, error.clone()),
        Some(RuleRefusal::Stated(message)) => TrialError::new_err(message.clone()),
        None => TrialError::new_err(format!(
            "the {slot} rule placed no landmark and gave no reason"
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
                    provenance: CoreProvenance {
                        method_id: method_id.to_string(),
                        bound_parameters,
                        registry_version: self.registry.version.clone(),
                        registry_digest: self.registry.digest.clone(),
                        acquisition_complete: self.acquisition_complete,
                    },
                },
                Vec::new(),
                depends_on,
            )
        })
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
}

#[pymethods]
impl CountermovementJump {
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
    weighing_epoch,
    onset,
    takeoff,
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
))]
#[allow(clippy::too_many_arguments)]
pub fn analyse_countermovement_jump(
    python: Python<'_>,
    trial: &Trial,
    weighing_epoch: &BoundMethod,
    onset: &BoundMethod,
    takeoff: &BoundMethod,
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
) -> PyResult<CountermovementJump> {
    expect_bound(weighing_epoch, "weighing")?;
    expect_bound(onset, "onset")?;
    expect_bound(takeoff, "takeoff")?;

    let registry = weighing_epoch.registry_identity().clone();
    let acquisition_complete = trial.acquisition_complete();

    let request = AnalysisRequest {
        weighing: WeighingChoice {
            method_id: weighing_epoch.method_id().to_string(),
            start_index: weighing_start_index,
            parameters: quantities_of(weighing_epoch, weighing_parameters),
            options: weighing_options.unwrap_or_default(),
        },
        onset: MethodChoice {
            manual_index: onset_index,
            ..choice_of(onset, onset_parameters, onset_options)
        },
        takeoff: MethodChoice {
            manual_index: takeoff_index,
            ..choice_of(takeoff, takeoff_parameters, takeoff_options)
        },
        touchdown_index,
        gravity_meters_per_second_squared,
        // What this registry carries. The binding composes operators onto the rule the
        // caller named, and those are entries in their own right that have to be judged
        // against the same list rather than assumed.
        registry_backed_ids: registry.method_ids.as_ref().clone(),
    };

    let response = plateforce_analysis::run(&trial.inner, &request).map_err(TrialError::new_err)?;

    let onset_index = response
        .onset_index
        .ok_or_else(|| refusal_of(python, &response, "onset"))?;
    let takeoff_index = response
        .takeoff_index
        .ok_or_else(|| refusal_of(python, &response, "takeoff"))?;

    let epoch_chain = chain_of(
        resolved_slot(&response, weighing_epoch.method_id()),
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
        resolved_slot(&response, onset.method_id()),
        &registry,
        acquisition_complete,
        onset_inputs,
    );
    let takeoff_chain = chain_of(
        resolved_slot(&response, takeoff.method_id()),
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
                bound_parameters: gravity_parameter,
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
            .flat_map(|bound| bound.assumed_parameters.iter().cloned())
            .collect(),
        warnings: response.warnings.clone(),
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
                method_id: JUMP_HEIGHT_FROM_FLIGHT_TIME_METHOD_ID.to_string(),
                bound_parameters: vec![
                    ("flight_time_seconds".to_string(), flight_time_seconds),
                    (
                        "gravity_meters_per_second_squared".to_string(),
                        gravity_meters_per_second_squared,
                    ),
                ],
                registry_version,
                registry_digest: None,
                acquisition_complete,
            },
        },
        Vec::new(),
        Vec::new(),
    )
}
