//! How far the method choice moves one number.
//!
//! The sweep is the measurement this software exists to publish: across ten published jump
//! height methods on 244 real trials the median spread is 3.51 cm, against the 1.98 cm
//! training effect the source study was built to detect. A notebook could not compute it
//! before this file, which left the largest population of readers in this field able to run
//! our analysis and unable to run our argument.
//!
//! Nothing here sweeps anything. `plateforce_analysis::spread` does, for every surface, and
//! this shapes its answer into the classes a notebook reads.

use std::collections::BTreeMap;

use plateforce_analysis::{bindings_for, spread};
use pyo3::prelude::*;

use crate::analysis::analysis_request_of;
use crate::errors::{raise_refusal, refusal_object, MethodError};
use crate::registry::{BoundMethod, Preset};
use crate::trial::Trial;

/// One combination that ran, and what it produced.
#[pyclass(
    frozen,
    skip_from_py_object,
    module = "plateforce",
    name = "SpreadVariant"
)]
#[derive(Clone)]
pub struct SpreadVariant {
    /// What was varied to reach this combination, as a reader would say it. "baseline" for
    /// the combination that varied nothing.
    #[pyo3(get)]
    label: String,
    /// The number this combination produced, or None where a rule on the quantity's chain
    /// declined. A variant that produced nothing stays in the denominator.
    #[pyo3(get)]
    value: Option<f64>,
    #[pyo3(get)]
    method_ids: Vec<String>,
    settings: Vec<(String, String)>,
    failure_reason: Option<plateforce_core::Refusal>,
}

#[pymethods]
impl SpreadVariant {
    /// What this combination bound, keyed by the name the registry publishes.
    #[getter]
    fn settings(&self) -> BTreeMap<String, String> {
        self.settings.iter().cloned().collect()
    }

    /// Why this combination produced no number, as the record the declining rule built,
    /// carrying `code` and every field beside it.
    ///
    /// An instance rather than a raise: a rule declining on one combination while the rest
    /// of the sweep computes is a partial result, not a failed one. It is an instance of the
    /// same class a caller would catch, so `isinstance` reads the same either way.
    #[getter]
    fn failure_reason(&self, python: Python<'_>) -> Option<Py<PyAny>> {
        self.failure_reason
            .as_ref()
            .map(|refusal| refusal_object(python, refusal))
    }

    fn __repr__(&self) -> String {
        match self.value {
            Some(value) => format!("SpreadVariant('{}', value={:.6})", self.label, value),
            None => format!("SpreadVariant('{}', declined)", self.label),
        }
    }
}

/// One quantity swept over a slot's defensible alternatives.
#[pyclass(frozen, skip_from_py_object, module = "plateforce", name = "Spread")]
pub struct Spread {
    /// Which build produced this sweep. A spread nested in an analysis inherits that
    /// result's identity; one that leaves on its own carried none, so a reader holding a
    /// spread could not say which software or which registry produced it.
    #[pyo3(get)]
    plateforce_version: String,
    /// The revision the caller pinned, and None where nobody pinned one. The same question
    /// `Registry.version` answers, which is not the one `Registry.declared_version` answers.
    #[pyo3(get)]
    registry_version: Option<String>,
    /// Identifies the registry files this sweep read, whether or not anybody declared a
    /// revision.
    #[pyo3(get)]
    registry_digest: Option<String>,
    #[pyo3(get)]
    quantity_key: String,
    #[pyo3(get)]
    unit: String,
    #[pyo3(get)]
    unit_symbol: String,
    /// How many combinations the axes describe, before any cap.
    #[pyo3(get)]
    combinations_requested: usize,
    #[pyo3(get)]
    combinations_run: usize,
    /// True when the cap stopped the sweep short, so the figures below describe part of
    /// what was asked for.
    #[pyo3(get)]
    capped: bool,
    #[pyo3(get)]
    succeeded: usize,
    #[pyo3(get)]
    failed: usize,
    #[pyo3(get)]
    minimum: Option<f64>,
    #[pyo3(get)]
    maximum: Option<f64>,
    #[pyo3(get)]
    median: Option<f64>,
    #[pyo3(get)]
    spread_absolute: Option<f64>,
    /// The headline figure. On the 244-trial corpus this reads 38.9 percent for time to
    /// takeoff, which is the whole argument for the registry in one number.
    #[pyo3(get)]
    spread_percent_of_median: Option<f64>,
    /// What the unswept request produced, so a reader can see where their own choice sits
    /// among the alternatives.
    #[pyo3(get)]
    baseline_value: Option<f64>,
    variants: Vec<SpreadVariant>,
}

#[pymethods]
impl Spread {
    /// Every combination that ran, each with its label, its value and the reason it declined
    /// where it did.
    #[getter]
    fn variants(&self) -> Vec<SpreadVariant> {
        self.variants.clone()
    }

    fn __len__(&self) -> usize {
        self.variants.len()
    }

    fn __repr__(&self) -> String {
        let percent = match self.spread_percent_of_median {
            Some(percent) => format!("{percent:.1}%"),
            None => "none".to_string(),
        };
        format!(
            "Spread('{}', succeeded={} of {}, spread_percent_of_median={})",
            self.quantity_key, self.succeeded, self.combinations_run, percent
        )
    }
}

/// Sweep a slot's alternatives and report how far the number moves.
///
/// This is what answers "how much does the method choice move this number", so it takes no
/// option to enable it and sits beside `analyse_countermovement_jump` rather than behind it.
///
/// `slot` is one name or several. Several sweeps every combination of them, which is the
/// question a reader asks about a number resting on more than one rule: the choice of onset
/// rule and the choice of takeoff rule both move a jump height, and sweeping them one at a
/// time reports neither the widest disagreement nor the narrowest.
///
/// Naming neither `method_ids` nor `parameter` sweeps every rule this build runs for the
/// slot, read off the binding table rather than from a list written here. Naming `parameter`
/// and `values` sweeps that parameter instead, holding the rule, and both describe one slot
/// so both are refused beside several.
///
/// The three rule arguments and `preset` are the ones `analyse_countermovement_jump` takes,
/// under the same names, because the sweep varies the request that call sends.
#[pyfunction]
// The Rust name differs because this crate's module is called `spread`; the name a caller
// types is the one every surface uses for this operation.
#[pyo3(name = "spread")]
#[pyo3(signature = (
    trial,
    quantity,
    slot,
    weighing_epoch = None,
    onset = None,
    takeoff = None,
    preset = None,
    method_ids = None,
    parameter = None,
    values = None,
    gravity_meters_per_second_squared = None,
    weighing_parameters = None,
    onset_parameters = None,
    takeoff_parameters = None,
    maximum_combinations = None,
))]
#[allow(clippy::too_many_arguments)]
pub fn spread_over(
    python: Python<'_>,
    trial: &Trial,
    quantity: &str,
    slot: &Bound<'_, PyAny>,
    weighing_epoch: Option<&BoundMethod>,
    onset: Option<&BoundMethod>,
    takeoff: Option<&BoundMethod>,
    preset: Option<&Preset>,
    method_ids: Option<Vec<String>>,
    parameter: Option<String>,
    values: Option<Vec<f64>>,
    gravity_meters_per_second_squared: Option<f64>,
    weighing_parameters: Option<BTreeMap<String, f64>>,
    onset_parameters: Option<BTreeMap<String, f64>>,
    takeoff_parameters: Option<BTreeMap<String, f64>>,
    maximum_combinations: Option<usize>,
) -> PyResult<Spread> {
    // The base is built by the one request builder this surface has, so the combination that
    // varies nothing is the request a user's own analysis call sends and the sweep is around
    // their result rather than around one assembled here.
    let (base, registry) = analysis_request_of(
        python,
        weighing_epoch,
        onset,
        takeoff,
        preset,
        gravity_meters_per_second_squared,
        weighing_parameters,
        onset_parameters,
        takeoff_parameters,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    )?;

    let request = spread::SpreadRequest {
        base,
        axes: axes_of(slot, method_ids, parameter, values)?,
        quantity_key: quantity.to_string(),
        maximum_combinations: maximum_combinations.unwrap_or(DEFAULT_MAXIMUM_COMBINATIONS),
    };

    let response =
        spread::run(&trial.inner, &request).map_err(|refusal| raise_refusal(python, &refusal))?;

    Ok(Spread {
        // Read off the rules this call named, the same identity `analyse_countermovement_jump`
        // stamps on its own record, rather than a second reading of the registry here.
        plateforce_version: env!("CARGO_PKG_VERSION").to_string(),
        registry_version: registry.stamp.version.clone(),
        registry_digest: registry.stamp.digest.clone(),
        quantity_key: response.quantity_key,
        unit: response.unit,
        unit_symbol: response.unit_symbol,
        combinations_requested: response.combinations_requested,
        combinations_run: response.combinations_run,
        capped: response.capped,
        succeeded: response.succeeded,
        failed: response.failed,
        minimum: response.minimum,
        maximum: response.maximum,
        median: response.median,
        spread_absolute: response.spread_absolute,
        spread_percent_of_median: response.spread_percent_of_median,
        baseline_value: response.baseline_value,
        variants: response
            .variants
            .into_iter()
            .map(|variant| SpreadVariant {
                label: variant.label,
                value: variant.value,
                method_ids: variant.method_ids,
                settings: variant.settings,
                failure_reason: variant.failure_reason,
            })
            .collect(),
    })
}

/// The engine's own cap, restated here because this signature offers it and a caller who
/// states nothing has to get the same sweep the other surfaces give them.
const DEFAULT_MAXIMUM_COMBINATIONS: usize = 512;

/// The dimensions to sweep, one per slot named.
///
/// Naming neither the rules nor a parameter sweeps every rule the build runs for the slot,
/// which is the question a reader asks first.
fn axes_of(
    slot: &Bound<'_, PyAny>,
    method_ids: Option<Vec<String>>,
    parameter: Option<String>,
    values: Option<Vec<f64>>,
) -> PyResult<Vec<spread::Axis>> {
    let named = slots_named(slot)?;
    if named.len() > 1 && (parameter.is_some() || method_ids.is_some()) {
        return Err(MethodError::new_err(
            "parameter and method_ids each describe one slot, so name one slot or neither"
                .to_string(),
        ));
    }
    named
        .iter()
        .map(|slot| axis_of(slot, method_ids.clone(), parameter.clone(), values.clone()))
        .collect()
}

/// One name or several, because a caller sweeping a single slot should not have to write a
/// list of one and a caller sweeping three should not have to call three times.
fn slots_named(slot: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    if let Ok(single) = slot.extract::<String>() {
        return Ok(vec![single]);
    }
    let several: Vec<String> = slot.extract().map_err(|_| {
        MethodError::new_err("slot is the name of a step, or several names".to_string())
    })?;
    if several.is_empty() {
        return Err(MethodError::new_err(
            "no slot was named, so there is nothing to sweep".to_string(),
        ));
    }
    Ok(several)
}

fn axis_of(
    slot: &str,
    method_ids: Option<Vec<String>>,
    parameter: Option<String>,
    values: Option<Vec<f64>>,
) -> PyResult<spread::Axis> {
    if parameter.is_some() {
        return Ok(spread::Axis {
            slot: slot.to_string(),
            parameter,
            values: values.unwrap_or_default(),
            method_ids: Vec::new(),
        });
    }

    let ids = match method_ids {
        Some(named) => named,
        None => bindings_for(slot)
            .map(|binding| binding.id.to_string())
            .collect(),
    };
    if ids.is_empty() {
        return Err(MethodError::new_err(format!(
            "this build runs no rule for {slot}, so there is nothing to sweep"
        )));
    }
    Ok(spread::Axis {
        slot: slot.to_string(),
        parameter: None,
        values: Vec::new(),
        method_ids: ids,
    })
}
