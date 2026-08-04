//! Running bound methods over a trial.
//!
//! Nothing here decides a method, resolves a parameter or computes a quantity.
//! `plateforce_analysis` does all three, for every surface. This file turns a
//! registry-bound method into the request that layer takes, and shapes what comes back
//! into the chain of choices a Python caller reads.

use std::collections::{BTreeMap, BTreeSet};

use plateforce_analysis::{
    bindings_for, chain_of, AnalysisRequest, AnalysisResponse, MethodChoice, Metric, WeighingChoice,
};
use plateforce_core::{
    jump_height_from_flight_time as core_jump_height_from_flight_time, Measured as CoreMeasured,
    Provenance as CoreProvenance,
};
use pyo3::prelude::*;

use crate::errors::{raise_refusal, MethodNotImplementedError, TrialError};
use crate::quality::QualitySignal;
use crate::registry::{BoundMethod, Preset, RegistryIdentity};
use crate::result::{Exclusions, Measured};
use crate::trial::Trial;

/// The registry entry behind the height a flight time alone gives, for the entry point that
/// takes a flight time from a contact mat and reads no response.
const JUMP_HEIGHT_FROM_FLIGHT_TIME_METHOD_ID: &str = "jumpheight.takeoff.flight_time";

/// Steps the software performs that no registry entry describes, reported on every result
/// rather than left to be discovered.
// Five of these are registry entries and were being reported as unregistered on this surface
// alone. The rule was always the registry's; only the name this crate used for it was not, so
// the same number arrived carrying a resolvable id through the browser and an unresolvable one
// through Python. That is a parity break on the exact property the product exists to guarantee,
// and it is worse than either surface being uniformly wrong.
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

/// The registry entry's own parameters, plus any the caller stated directly, and the names
/// among them that came from the entry rather than from the caller. A name no rule reads is
/// not dropped in silence: it comes back in `unread_parameters`.
///
/// Both together, never the values alone. A binding's `bound_parameters` carries the entry's
/// defaults beside the caller's own values and the two are indistinguishable there, so a
/// request built from it and nothing else told the engine that every one of them was chosen.
/// `stated_source` in plateforce-analysis then recorded a registry default as the reader's
/// own decision, which is the one claim this software exists to get right. Measured on
/// `tests/golden/result-parity-request-inverted.json`: three parameters no caller named,
/// among them a takeoff rule's `k` and an onset rule's `window_seconds`, came back as
/// `stated` here while the terminal, R and the browser each said `assumed`.
fn quantities_of(
    method: &BoundMethod,
    stated: Option<BTreeMap<String, f64>>,
) -> (BTreeMap<String, f64>, BTreeSet<String>) {
    let stated = stated.unwrap_or_default();
    // A name the caller stated is theirs even where the entry publishes a default for it,
    // because `bind` records a default only where nothing was supplied.
    let from_registry_default: BTreeSet<String> = method
        .names_the_registry_filled()
        .iter()
        .filter(|name| !stated.contains_key(*name))
        .cloned()
        .collect();
    let mut parameters: BTreeMap<String, f64> = method.bound_parameters.iter().cloned().collect();
    parameters.extend(stated);
    (parameters, from_registry_default)
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
    let (parameters, from_registry_default) = quantities_of(method, parameters);
    MethodChoice {
        method_id: method.method_id().to_string(),
        parameters,
        options: options.unwrap_or_default(),
        manual_index: None,
        from_registry_default,
        ..Default::default()
    }
}

/// What one call says about the phase that conditions the signal, keyed by the construct the
/// registry declares.
///
/// The keys are the union of the three arguments, because a caller may name a rule for the
/// phase, state values against the rule it runs anyway, or both. A construct none of them
/// names is left out rather than sent as an empty choice: the phase runs it either way and
/// leaves the same record, so a key in the map is the caller having spoken.
///
/// A construct written against with no rule named carries no id, which is the engine's word
/// for a caller who stated values and left the rule to the phase.
fn conditioning_choices(
    rules: &BTreeMap<String, BoundMethod>,
    parameters: &BTreeMap<String, BTreeMap<String, f64>>,
    options: &BTreeMap<String, BTreeMap<String, String>>,
) -> BTreeMap<String, MethodChoice> {
    let mut constructs: BTreeSet<&String> = BTreeSet::new();
    constructs.extend(rules.keys());
    constructs.extend(parameters.keys());
    constructs.extend(options.keys());
    constructs
        .into_iter()
        .map(|construct| {
            (
                construct.clone(),
                unbound_or(
                    rules.get(construct),
                    parameters.get(construct).cloned(),
                    options.get(construct).cloned(),
                ),
            )
        })
        .collect()
}

/// The weighing slot's choice, taken from the one place every slot's choice is built.
///
/// Destructured without a rest pattern, so a claim `MethodChoice` gains is a compile error here
/// rather than one the weighing slot quietly stops sending. This arm used to be written out
/// beside `choice_of` and end in `..Default::default()`, which is one slot's worth of the
/// request assembled twice.
fn weighing_choice(chosen: MethodChoice, start_index: Option<usize>) -> WeighingChoice {
    // A weighing window placed by hand travels as `start_index`, which is the argument this
    // takes, and `choice_of` sets no dragged index on any slot.
    let MethodChoice {
        method_id,
        parameters,
        options,
        manual_index: _,
        recommended,
        method_from_recommendation,
        method_from_registry_default,
        from_registry_default,
        cited,
        preset,
    } = chosen;
    WeighingChoice {
        method_id,
        start_index,
        parameters,
        options,
        recommended,
        method_from_recommendation,
        method_from_registry_default,
        from_registry_default,
        cited,
        preset,
    }
}

/// The quantities whose value moves when the gravity the analysis was bound to moves.
///
/// A rule may only record a parameter its own registry entry declares, and of the twelve rules
/// that read this value one declares it, so for these five the number that moved them reaches
/// no rule's row. `AnalysisResponse::bound_globals` carries it once for the whole analysis; a
/// `Measured` travels away from the result it came out of, so it carries it as well.
///
/// `tests/test_gravity_record.py` measures which numbers move and holds this list to the
/// measurement, in both directions.
const QUANTITIES_RESTING_ON_THE_ANALYSIS_GRAVITY: &[&str] = &[
    "jump_height_from_flight_time_meters",
    "jump_height_from_takeoff_meters",
    "reactive_strength_index_modified",
    "system_mass_kilograms",
    "takeoff_velocity_meters_per_second",
];

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
    /// What the request bound for the whole analysis, which no rule's row can carry because
    /// no rule's entry declares it.
    analysis_gravity: (f64, plateforce_core::provenance::ParameterSource),
}

impl Derived<'_> {
    /// From the quantity declaration rather than from the result, so a key that produced no
    /// value on this trial still reports the unit it would have been in.
    fn unit(&self, key: &str) -> &'static str {
        plateforce_analysis::response::quantity(key)
            .map(|declared| declared.unit)
            .unwrap_or_default()
    }

    /// One number and the chain of rules behind it, or nothing where this analysis reported
    /// no number under that name.
    ///
    /// The one route to a record on this surface. `value()` and each named getter come
    /// through here, so which of the two a caller asked through cannot change what the
    /// record says. It used to: a getter returned a hand-assembled chain naming the whole
    /// pipeline and `value()` returned the same quantity with a step carrying no parameters
    /// and no inputs at all.
    ///
    /// The tree itself is `plateforce_analysis::chain_of`'s, which is where every surface
    /// reads it.
    fn one(&self, key: &str) -> Option<Measured> {
        let metric = self.response.metric(key)?;
        let value = metric.value?;
        let mut chain = chain_of(
            self.response,
            metric,
            &self.registry.stamp,
            self.acquisition_complete,
        );
        chain
            .provenance
            .parameters
            .extend(self.gravity_behind(metric));
        Some(Measured::new(
            CoreMeasured {
                value,
                unit: self.unit(key),
                provenance: chain.provenance,
            },
            chain.enumerated_choices,
            chain.depends_on,
        ))
    }

    /// The gravity one number ran under, for the quantities that move with it, read off the
    /// record rather than off the request.
    ///
    /// A rule whose registry entry publishes a gravity of its own records it on its own row
    /// and may have run at a value the request never held. `jumpheight.takeoff.flight_time`
    /// is that rule: on a request nobody stated a gravity for it runs at the 9.81 its entry
    /// declares while the request carries 9.80665, so its own row and the analysis value are
    /// two different numbers and only one of them produced the height.
    fn gravity_behind(&self, metric: &Metric) -> Vec<plateforce_core::provenance::ParameterRecord> {
        use plateforce_analysis::slots::jh_takeoff_frame::flight_time::GRAVITY_PARAMETER;
        use plateforce_core::provenance::{ParameterRecord, ParameterSource};

        if !QUANTITIES_RESTING_ON_THE_ANALYSIS_GRAVITY.contains(&metric.key.as_str()) {
            return Vec::new();
        }
        let published_by_the_rule = metric
            .computed_by
            .as_deref()
            .and_then(|id| {
                self.response
                    .bound_methods
                    .iter()
                    .find(|bound| bound.method_id == id)
            })
            .and_then(|bound| {
                let value = *bound.numeric_values.get(GRAVITY_PARAMETER)?;
                let source = bound
                    .parameter_sources
                    .get(GRAVITY_PARAMETER)
                    .copied()
                    .unwrap_or(ParameterSource::Assumed);
                Some((value, source))
            });
        let (value, source) = published_by_the_rule.unwrap_or(self.analysis_gravity);
        vec![ParameterRecord {
            name: plateforce_analysis::GRAVITY_GLOBAL.to_string(),
            value,
            source,
        }]
    }

    /// Every quantity the response reported a number for, keyed by the engine's own name for
    /// it, each carrying the record `one` built.
    ///
    /// Read through `value()` rather than through a getter per quantity. Eleven getters
    /// were written when eleven quantities existed, and a rule bound for any other
    /// construct reports a key none of them names, so a transcription would go stale the
    /// first time one landed.
    fn every_value(&self) -> BTreeMap<String, Measured> {
        self.response
            .metrics
            .iter()
            .filter_map(|metric| self.one(&metric.key).map(|held| (metric.key.clone(), held)))
            .collect()
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
    /// What the software noticed about the values above, as the records the engine raised.
    signals: Vec<QualitySignal>,
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

    /// What the software noticed about the values above.
    ///
    /// A signal is not a refusal and not a warning: the number stands, and each one carries
    /// the action a reader would take rather than a verdict about the trace. An empty list
    /// is a result nothing was noticed about.
    #[getter]
    fn signals(&self) -> Vec<QualitySignal> {
        self.signals.clone()
    }

    /// The signals about one quantity, by the engine's name for it.
    ///
    /// Each signal declares which quantities it qualifies, so this reads that declaration
    /// rather than a lookup table kept beside it, and a signal cannot drift away from the
    /// value it is about. The browser places its signals the same way.
    fn signals_qualifying(&self, quantity: &str) -> Vec<QualitySignal> {
        self.signals
            .iter()
            .filter(|signal| signal.qualifies_key(quantity))
            .cloned()
            .collect()
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
    gravity_meters_per_second_squared: Option<f64>,
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
    derived_options: Option<BTreeMap<String, BTreeMap<String, String>>>,
    conditioning: Option<BTreeMap<String, Py<BoundMethod>>>,
    conditioning_parameters: Option<BTreeMap<String, BTreeMap<String, f64>>>,
    conditioning_options: Option<BTreeMap<String, BTreeMap<String, String>>>,
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
    let derived_options = derived_options.unwrap_or_default();
    expect_derived_bound(python, &derived)?;

    // The phase that conditions the signal runs on every analysis, so until this landed a
    // notebook reported the software's answer about it on every run and had no way to state
    // its own. The three arguments are read into one map here and checked before the trial
    // is touched.
    let conditioning_rules: BTreeMap<String, BoundMethod> = conditioning
        .unwrap_or_default()
        .into_iter()
        .map(|(construct, method)| (construct, method.borrow(python).clone()))
        .collect();
    let conditioning = conditioning_choices(
        &conditioning_rules,
        &conditioning_parameters.unwrap_or_default(),
        &conditioning_options.unwrap_or_default(),
    );
    for (construct, choice) in &conditioning {
        plateforce_analysis::binding::accepts_conditioning(construct, &choice.method_id)
            .map_err(|refusal| raise_refusal(python, &refusal))?;
    }

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

    // The value and the claim about where it came from are written together, by the one
    // routine every surface writes a gravity through.
    let (gravity_meters_per_second_squared, gravity_source) =
        plateforce_analysis::gravity_stated(gravity_meters_per_second_squared);

    let mut request = AnalysisRequest {
        weighing: weighing_choice(
            unbound_or(weighing_epoch, weighing_parameters, weighing_options),
            weighing_start_index,
        ),
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
        gravity_source,
        // What this registry carries. The binding composes operators onto the rule the
        // caller named, and those are entries in their own right that have to be judged
        // against the same list rather than assumed.
        registry_backed_ids: registry.method_ids.as_ref().clone(),
        conditioning,
        // A rule computed from the landmarks reads the enumerations its entry declares, the
        // same as one on the path, and the folder call has been able to state them since it
        // gained the argument. One trial could not, so a construct whose rule turns on a
        // named choice ran under whatever the registry binds when nobody chooses, and the
        // record said assumed while the caller was holding the choice they wanted.
        derived: derived
            .iter()
            .map(|(construct, method)| {
                (
                    construct.clone(),
                    choice_of(
                        method,
                        derived_parameters.get(construct).cloned(),
                        derived_options.get(construct).cloned(),
                    ),
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
    /// The revision the caller pinned on the registry they loaded, and null when they pinned
    /// none. Never the registry's own claim, which travels beside it.
    registry_version: Option<String>,
    registry_declared_version: Option<String>,
    acquisition_complete: bool,
    /// The account each quantity gives of itself, keyed by the quantity. Generated by
    /// `plateforce_analysis::descriptions_of` so a number reads the same in a notebook, a
    /// terminal, a browser tab and an R session. A notebook already reads one per value
    /// through `Measured.describe`; the document it hands to another reader carried none.
    descriptions: BTreeMap<String, String>,
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
    gravity_meters_per_second_squared = None,
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
    derived_options = None,
    conditioning = None,
    conditioning_parameters = None,
    conditioning_options = None,
))]
#[allow(clippy::too_many_arguments)]
pub fn analyse_json(
    python: Python<'_>,
    trial: &Trial,
    weighing_epoch: Option<&BoundMethod>,
    onset: Option<&BoundMethod>,
    takeoff: Option<&BoundMethod>,
    preset: Option<&Preset>,
    gravity_meters_per_second_squared: Option<f64>,
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
    derived_options: Option<BTreeMap<String, BTreeMap<String, String>>>,
    conditioning: Option<BTreeMap<String, Py<BoundMethod>>>,
    conditioning_parameters: Option<BTreeMap<String, BTreeMap<String, f64>>>,
    conditioning_options: Option<BTreeMap<String, BTreeMap<String, String>>>,
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
        derived_options,
        conditioning,
        conditioning_parameters,
        conditioning_options,
    )?;

    let response = plateforce_analysis::run(&trial.inner, &request)
        .map_err(|refusal| raise_refusal(python, &refusal))?;

    let acquisition_complete = trial.acquisition_complete();
    let document = AnalysisDocument {
        descriptions: plateforce_analysis::accounts_of(
            &response,
            &registry.stamp,
            acquisition_complete,
        ),
        response: &response,
        registry_digest: registry.stamp.digest.clone(),
        registry_version: registry.stamp.version.clone(),
        registry_declared_version: registry.stamp.declared_version.clone(),
        acquisition_complete,
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
/// The `conditioning` arguments say what produced the signal every landmark was placed on,
/// keyed by the construct the registry declares. The phase runs whether or not they are
/// passed, so what they buy is the record naming the caller rather than the software: a rule
/// for the phase in `conditioning`, and the values it reads in `conditioning_parameters` and
/// `conditioning_options`, which state values against the rule the phase runs anyway.
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
    gravity_meters_per_second_squared = None,
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
    derived_options = None,
    conditioning = None,
    conditioning_parameters = None,
    conditioning_options = None,
))]
#[allow(clippy::too_many_arguments)]
pub fn analyse_countermovement_jump(
    python: Python<'_>,
    trial: &Trial,
    weighing_epoch: Option<&BoundMethod>,
    onset: Option<&BoundMethod>,
    takeoff: Option<&BoundMethod>,
    preset: Option<&Preset>,
    gravity_meters_per_second_squared: Option<f64>,
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
    derived_options: Option<BTreeMap<String, BTreeMap<String, String>>>,
    conditioning: Option<BTreeMap<String, Py<BoundMethod>>>,
    conditioning_parameters: Option<BTreeMap<String, BTreeMap<String, f64>>>,
    conditioning_options: Option<BTreeMap<String, BTreeMap<String, String>>>,
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
        derived_options,
        conditioning,
        conditioning_parameters,
        conditioning_options,
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

    let derived = Derived {
        response: &response,
        registry: &registry,
        acquisition_complete,
        analysis_gravity: (
            request.gravity_meters_per_second_squared,
            request.gravity_source,
        ),
    };
    // A quantity the spine always reports, and the slot whose rule declined when it is
    // missing, so a caller meets the refusal that rule made rather than an absent attribute.
    let required = |key: &str, slot: &str| {
        derived
            .one(key)
            .ok_or_else(|| refusal_of(python, &response, slot))
    };

    Ok(CountermovementJump {
        system_weight_newtons: required("system_weight_newtons", "weighing")?,
        system_mass_kilograms: required("system_mass_kilograms", "weighing")?,
        weighing_epoch_tied_window_count: response.weighing_epoch_tied_window_count,
        onset_index,
        onset_time_seconds: required("onset_time_seconds", "onset")?,
        takeoff_index,
        takeoff_time_seconds: required("takeoff_time_seconds", "takeoff")?,
        touchdown_index: response.touchdown_index,
        time_to_takeoff_seconds: required("time_to_takeoff_seconds", "onset")?,
        flight_time_seconds: derived.one("flight_time_seconds"),
        net_impulse_newton_seconds: required("net_impulse_newton_seconds", "takeoff")?,
        takeoff_velocity_meters_per_second: required(
            "takeoff_velocity_meters_per_second",
            "takeoff",
        )?,
        jump_height_takeoff_frame_meters: required("jump_height_from_takeoff_meters", "takeoff")?,
        jump_height_flight_time_meters: derived.one("jump_height_from_flight_time_meters"),
        reactive_strength_index_modified: derived.one("reactive_strength_index_modified"),
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
        // The values this analysis was bound to are read alongside the rules', because a
        // gravity nobody was asked about is a value nobody chose in exactly the sense this
        // list reports, and no rule's row can carry it.
        assumed_parameters: response
            .bound_methods
            .iter()
            .flat_map(|bound| bound.assumed_parameters())
            .chain(
                response
                    .bound_globals
                    .iter()
                    .filter(|bound| {
                        bound.source == plateforce_core::provenance::ParameterSource::Assumed
                    })
                    .map(|bound| bound.name.to_string()),
            )
            .collect(),
        warnings: response.warnings.clone(),
        // The signals the analysis already raised. Raising them again here would run the
        // same function over the same response a second time, and a signal that disagreed
        // with the number it qualifies would be worse than no signal at all.
        signals: response
            .signals
            .iter()
            .cloned()
            .map(QualitySignal::of)
            .collect(),
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
    gravity_meters_per_second_squared = None,
    registry_version = None,
    acquisition_complete = false,
))]
pub fn jump_height_from_flight_time(
    flight_time_seconds: f64,
    gravity_meters_per_second_squared: Option<f64>,
    registry_version: Option<String>,
    acquisition_complete: bool,
) -> Measured {
    use plateforce_core::provenance::{ParameterRecord, ParameterSource};
    let (gravity_meters_per_second_squared, gravity_source) =
        plateforce_analysis::gravity_stated(gravity_meters_per_second_squared);
    Measured::new(
        CoreMeasured {
            value: core_jump_height_from_flight_time(
                flight_time_seconds,
                gravity_meters_per_second_squared,
            ),
            unit: "meters",
            provenance: CoreProvenance {
                // The flight time was measured off a trace the caller holds; the gravity is
                // whatever they said, or the constant nobody asked them about. One record
                // giving both the same source claimed a measurement of the second.
                parameters: vec![
                    ParameterRecord {
                        name: "flight_time_seconds".to_string(),
                        value: flight_time_seconds,
                        source: ParameterSource::Measured,
                    },
                    ParameterRecord {
                        name: plateforce_analysis::GRAVITY_GLOBAL.to_string(),
                        value: gravity_meters_per_second_squared,
                        source: gravity_source,
                    },
                ],
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

#[cfg(test)]
mod tests {
    use super::*;

    use plateforce_core::provenance::ParameterSource;
    use plateforce_core::Trial;

    const SAMPLE_RATE_HZ: f64 = 1200.0;

    /// The rules the committed inverted parity request binds, which is the request that first
    /// separated this surface from the other three. Every one of them publishes a default for
    /// a name that request leaves unstated.
    const WEIGHING_RULE: &str = "bwepoch.adaptive_lowest_variance";
    const ONSET_RULE: &str = "onset.threshold.adaptive_trailing_window";
    const TAKEOFF_RULE: &str = "takeoff.threshold.flight_noise_k_sd";

    /// A countermovement jump that leaves the plate and lands back on it, so the weighing rule
    /// has a quiet stretch to search and every landmark is placed.
    fn a_jump_that_lands() -> Trial {
        let mut force = vec![600.0; 1200];
        for (index, sample) in force.iter_mut().enumerate() {
            *sample += ((index % 17) as f64 - 8.0) * 0.4;
        }
        force.extend((0..240).map(|index| 600.0 - 220.0 * (index as f64 / 240.0)));
        force.extend((0..240).map(|index| 380.0 + 220.0 * (index as f64 / 240.0)));
        force.extend((0..660).map(|index| 600.0 + 900.0 * (index as f64 / 660.0)));
        force.extend(std::iter::repeat_n(0.0, 811));
        force.extend(std::iter::repeat_n(2400.0, 240));
        force.extend(std::iter::repeat_n(600.0, 600));
        Trial::new(force, SAMPLE_RATE_HZ).expect("the fixture is long enough to analyse")
    }

    /// One analysis through the request this surface writes, with each rule bound the way a
    /// notebook binds it: the values the caller names ride on the binding and every other name
    /// takes the entry's own default.
    fn analysed(stated: BTreeMap<&str, BTreeMap<String, f64>>) -> AnalysisResponse {
        let bound = |id: &str| {
            crate::registry::bound_from_the_registry_this_build_carries(
                id,
                stated.get(id).cloned().unwrap_or_default(),
            )
        };
        let weighing = bound(WEIGHING_RULE);
        let onset = bound(ONSET_RULE);
        let takeoff = bound(TAKEOFF_RULE);
        let request = AnalysisRequest {
            weighing: weighing_choice(choice_of(&weighing, None, None), None),
            onset: choice_of(&onset, None, None),
            takeoff: choice_of(&takeoff, None, None),
            registry_backed_ids: weighing.registry_identity().method_ids.as_ref().clone(),
            ..Default::default()
        };
        plateforce_analysis::run(&a_jump_that_lands(), &request).expect("the request is bound")
    }

    fn source_of(response: &AnalysisResponse, method_id: &str, name: &str) -> ParameterSource {
        let bound = response
            .bound_methods
            .iter()
            .find(|bound| bound.method_id == method_id)
            .unwrap_or_else(|| panic!("{method_id} left no record on this response"));
        *bound
            .parameter_sources
            .get(name)
            .unwrap_or_else(|| panic!("{method_id} recorded no source for {name}"))
    }

    /// A value the registry filled in reaches the engine claiming nobody chose it.
    ///
    /// The binding carries the entry's defaults beside the caller's own values and the two are
    /// indistinguishable there, so a request built from the values alone told the engine that
    /// every one of them was chosen, and the engine recorded a default nobody was asked about
    /// as the reader's own decision.
    ///
    /// Both halves, on one rule and one run. A build answering `assumed` for everything
    /// satisfies the first assertion and fails the second, and one answering `stated` for
    /// everything fails the first, so neither passes by giving one answer.
    #[test]
    fn a_value_the_registry_filled_in_is_not_recorded_as_one_the_caller_stated() {
        let response = analysed(BTreeMap::from([(
            WEIGHING_RULE,
            BTreeMap::from([("window_seconds".to_string(), 1.0)]),
        )]));

        assert_eq!(
            source_of(
                &response,
                WEIGHING_RULE,
                "reject_at_or_below_fraction_of_weight"
            ),
            ParameterSource::Assumed,
            "a gate nobody stated is recorded as the caller's own decision"
        );
        assert_eq!(
            source_of(&response, WEIGHING_RULE, "window_seconds"),
            ParameterSource::Stated,
            "a window the caller named is recorded as one they did not"
        );
    }

    /// And on the two slots the weighing rule does not speak for, so the claim cannot reach one
    /// slot and be dropped on the others.
    #[test]
    fn every_slot_says_which_of_its_values_the_registry_filled_in() {
        let response = analysed(BTreeMap::new());

        for (rule, name) in [
            (WEIGHING_RULE, "window_seconds"),
            (ONSET_RULE, "k"),
            (TAKEOFF_RULE, "k"),
        ] {
            assert_eq!(
                source_of(&response, rule, name),
                ParameterSource::Assumed,
                "{rule} ran on the registry's own {name} and the record names the caller"
            );
        }
    }

    /// The control on the guard above: the same three names, stated, and the same three rules
    /// record the caller. A build that lost the claim reports `stated` in both places, and one
    /// that hard-coded it reports `assumed` in both.
    #[test]
    fn a_value_the_caller_named_is_recorded_as_theirs_on_every_slot() {
        let response = analysed(BTreeMap::from([
            (
                WEIGHING_RULE,
                BTreeMap::from([("window_seconds".to_string(), 0.5)]),
            ),
            (ONSET_RULE, BTreeMap::from([("k".to_string(), 3.0)])),
            (TAKEOFF_RULE, BTreeMap::from([("k".to_string(), 3.0)])),
        ]));

        for (rule, name) in [
            (WEIGHING_RULE, "window_seconds"),
            (ONSET_RULE, "k"),
            (TAKEOFF_RULE, "k"),
        ] {
            assert_eq!(
                source_of(&response, rule, name),
                ParameterSource::Stated,
                "{rule} ran on the caller's own {name} and the record names the registry"
            );
        }
    }
}
