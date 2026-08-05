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

/// One dimension a sweep varied, as the record of what was varied rather than the request.
#[pyclass(
    frozen,
    skip_from_py_object,
    module = "plateforce",
    name = "SpreadAxis"
)]
#[derive(Clone)]
pub struct SpreadAxis {
    /// The word the request reaches this step by.
    #[pyo3(get)]
    slot: String,
    /// The registry construct that word names, so a reader looks up what varied rather than a
    /// word that appears in no registry file.
    #[pyo3(get)]
    construct: String,
    /// Rules compared along this axis, or 0 where the axis varied a setting instead.
    #[pyo3(get)]
    rules_varied: usize,
    /// Values compared along this axis, numbers and names alike.
    #[pyo3(get)]
    values_varied: usize,
    /// The setting the values were written against, and None where the rule varied.
    #[pyo3(get)]
    parameter: Option<String>,
}

#[pymethods]
impl SpreadAxis {
    fn __repr__(&self) -> String {
        match (&self.parameter, self.rules_varied) {
            (Some(parameter), _) => format!(
                "SpreadAxis('{}.{parameter}', values={})",
                self.construct, self.values_varied
            ),
            (None, rules) => format!("SpreadAxis('{}', rules={rules})", self.construct),
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
    /// The revision the registry names about itself, and None where it names none. What the
    /// data claims, never what the caller cited, which is the pin above.
    #[pyo3(get)]
    registry_declared_version: Option<String>,
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
    axes_varied: Vec<SpreadAxis>,
    held_fixed: Vec<(String, String)>,
}

#[pymethods]
impl Spread {
    /// Every combination that ran, each with its label, its value and the reason it declined
    /// where it did.
    #[getter]
    fn variants(&self) -> Vec<SpreadVariant> {
        self.variants.clone()
    }

    /// What this sweep varied, one entry per axis.
    ///
    /// A spread is a number over a set of choices and a reader cannot judge it without the
    /// set. The terminal panel, the browser panel and the JSON document all carry this, and
    /// the class a notebook holds did not: a reader with a `Spread` in hand could read the
    /// figure and not what it was taken over.
    #[getter]
    fn axes_varied(&self) -> Vec<SpreadAxis> {
        self.axes_varied.clone()
    }

    /// The rules this request bound that no axis varied, keyed by the construct each was
    /// pinned to.
    ///
    /// The other half of the same question, so the figure cannot be read as wider than the
    /// set it came from. A mapping rather than the record's list of pairs, because a
    /// construct appears once and that is the shape a notebook indexes.
    #[getter]
    fn held_fixed(&self) -> BTreeMap<String, String> {
        self.held_fixed.iter().cloned().collect()
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
/// Naming neither `method_ids` nor `vary` sweeps every rule bound for the slot, read off
/// the binding table rather than from a list written here. `vary` maps a `slot.parameter`
/// name to the values to sweep, holding the rule, the same axis the terminal spells
/// `--vary onset.k=2,5,10`. Each axis describes one slot, so both arguments are refused
/// beside several.
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
    slot = None,
    weighing_epoch = None,
    onset = None,
    takeoff = None,
    preset = None,
    method_ids = None,
    vary = None,
    gravity_meters_per_second_squared = None,
    body_mass_kilograms = None,
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
    maximum_combinations = None,
))]
#[allow(clippy::too_many_arguments)]
pub fn spread_over(
    python: Python<'_>,
    trial: &Trial,
    quantity: &str,
    slot: Option<Bound<'_, PyAny>>,
    weighing_epoch: Option<&BoundMethod>,
    onset: Option<&BoundMethod>,
    takeoff: Option<&BoundMethod>,
    preset: Option<&Preset>,
    method_ids: Option<Bound<'_, PyAny>>,
    vary: Option<Bound<'_, PyAny>>,
    gravity_meters_per_second_squared: Option<f64>,
    body_mass_kilograms: Option<f64>,
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
    maximum_combinations: Option<usize>,
) -> PyResult<Spread> {
    let (response, stamp) = swept(
        python,
        trial,
        quantity,
        slot.as_ref(),
        method_ids.as_ref(),
        vary.as_ref(),
        RequestArguments {
            weighing_epoch,
            onset,
            takeoff,
            preset,
            gravity_meters_per_second_squared,
            body_mass_kilograms,
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
        },
        maximum_combinations,
    )?;

    Ok(Spread {
        // Read off the rules this call named, the same identity `analyse_countermovement_jump`
        // stamps on its own record, rather than a second reading of the registry here.
        plateforce_version: env!("CARGO_PKG_VERSION").to_string(),
        registry_version: stamp.version.clone(),
        registry_declared_version: stamp.declared_version.clone(),
        registry_digest: stamp.digest.clone(),
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
        axes_varied: response
            .axes_varied
            .iter()
            .map(|axis| SpreadAxis {
                slot: axis.slot.clone(),
                construct: axis.construct.clone(),
                rules_varied: axis.rules_varied,
                values_varied: axis.values_varied,
                parameter: axis.parameter.clone(),
            })
            .collect(),
        held_fixed: response
            .held_fixed
            .iter()
            .map(|rule| (rule.construct.clone(), rule.method_id.clone()))
            .collect(),
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

/// Every argument `analyse_countermovement_jump` takes, carried whole.
///
/// A sweep varies the request an analysis sends, so the two take one argument set or the
/// sweep asks a narrower question than the surface beside it can answer. Thirteen of these
/// reached the builder as `None` written thirteen times, and a notebook could sweep no
/// derived construct, no conditioning rule, no placed landmark and no name a rule takes.
///
/// One value rather than twenty-two more positional arguments through three functions. Three
/// same-typed maps in a row is a signature that accepts a transposed pair and compiles, which
/// is the fault `SpreadDocument::of` already names, and there are four such runs here.
pub(crate) struct RequestArguments<'a> {
    pub weighing_epoch: Option<&'a BoundMethod>,
    pub onset: Option<&'a BoundMethod>,
    pub takeoff: Option<&'a BoundMethod>,
    pub preset: Option<&'a Preset>,
    pub gravity_meters_per_second_squared: Option<f64>,
    pub body_mass_kilograms: Option<f64>,
    pub weighing_parameters: Option<BTreeMap<String, f64>>,
    pub onset_parameters: Option<BTreeMap<String, f64>>,
    pub takeoff_parameters: Option<BTreeMap<String, f64>>,
    pub weighing_options: Option<BTreeMap<String, String>>,
    pub onset_options: Option<BTreeMap<String, String>>,
    pub takeoff_options: Option<BTreeMap<String, String>>,
    pub weighing_start_index: Option<usize>,
    pub onset_index: Option<usize>,
    pub takeoff_index: Option<usize>,
    pub touchdown_index: Option<usize>,
    pub derived: Option<BTreeMap<String, Py<BoundMethod>>>,
    pub derived_parameters: Option<BTreeMap<String, BTreeMap<String, f64>>>,
    pub derived_options: Option<BTreeMap<String, BTreeMap<String, String>>>,
    pub conditioning: Option<BTreeMap<String, Py<BoundMethod>>>,
    pub conditioning_parameters: Option<BTreeMap<String, BTreeMap<String, f64>>>,
    pub conditioning_options: Option<BTreeMap<String, BTreeMap<String, String>>>,
}

/// One home for running a sweep, so the shaped answer and the engine's own record below
/// cannot come from two different requests.
///
/// The base is built by the one request builder this surface has, so the combination that
/// varies nothing is the request a user's own analysis call sends and the sweep is around
/// their result rather than around one assembled here.
// Eight, and the eighth is `RequestArguments`, which already folds thirteen of them. Grouping
// further would put the sweep's own axes behind a record whose only caller is this function.
#[allow(clippy::too_many_arguments)]
fn swept(
    python: Python<'_>,
    trial: &Trial,
    quantity: &str,
    slot: Option<&Bound<'_, PyAny>>,
    method_ids: Option<&Bound<'_, PyAny>>,
    vary: Option<&Bound<'_, PyAny>>,
    arguments: RequestArguments<'_>,
    maximum_combinations: Option<usize>,
) -> PyResult<(
    spread::SpreadResponse,
    plateforce_core::provenance::RegistryStamp,
)> {
    // Destructured without a rest pattern, so an argument added to the analysis reaches the
    // sweep or stops this compiling. Thirteen of them were absent here for exactly as long as
    // nothing forced the two lists to be the same length.
    let RequestArguments {
        weighing_epoch,
        onset,
        takeoff,
        preset,
        gravity_meters_per_second_squared,
        body_mass_kilograms,
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
    } = arguments;

    let (base, registry) = analysis_request_of(
        python,
        weighing_epoch,
        onset,
        takeoff,
        preset,
        gravity_meters_per_second_squared,
        body_mass_kilograms,
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

    let request = spread::SpreadRequest {
        base,
        axes: axes_of(python, slot, method_ids, vary)?,
        quantity_key: quantity.to_string(),
        maximum_combinations: maximum_combinations.unwrap_or(DEFAULT_MAXIMUM_COMBINATIONS),
    };

    let response =
        spread::run(&trial.inner, &request).map_err(|refusal| raise_refusal(python, &refusal))?;
    Ok((response, registry.stamp.clone()))
}

/// The engine's own record of one sweep, as the engine wrote it.
///
/// `spread` reshapes that record into the classes a notebook reads and keeps no copy of it,
/// so nothing on this surface could be handed to a comparison against another surface's
/// sweep. This returns it whole, through the same run the shaped call makes. The private
/// primitive `_analyse_json` is beside it for the analysed document and exists for this
/// reason, and `scripts/result-from-python.py` is what asks both.
#[pyfunction]
#[pyo3(name = "_spread_json")]
#[pyo3(signature = (
    trial,
    quantity,
    slot = None,
    weighing_epoch = None,
    onset = None,
    takeoff = None,
    preset = None,
    method_ids = None,
    vary = None,
    gravity_meters_per_second_squared = None,
    body_mass_kilograms = None,
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
    maximum_combinations = None,
))]
#[allow(clippy::too_many_arguments)]
pub fn spread_json(
    python: Python<'_>,
    trial: &Trial,
    quantity: &str,
    slot: Option<Bound<'_, PyAny>>,
    weighing_epoch: Option<&BoundMethod>,
    onset: Option<&BoundMethod>,
    takeoff: Option<&BoundMethod>,
    preset: Option<&Preset>,
    method_ids: Option<Bound<'_, PyAny>>,
    vary: Option<Bound<'_, PyAny>>,
    gravity_meters_per_second_squared: Option<f64>,
    body_mass_kilograms: Option<f64>,
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
    maximum_combinations: Option<usize>,
) -> PyResult<String> {
    let (response, stamp) = swept(
        python,
        trial,
        quantity,
        slot.as_ref(),
        method_ids.as_ref(),
        vary.as_ref(),
        RequestArguments {
            weighing_epoch,
            onset,
            takeoff,
            preset,
            gravity_meters_per_second_squared,
            body_mass_kilograms,
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
        },
        maximum_combinations,
    )?;
    let document = plateforce_analysis::document::SpreadDocument::of(
        env!("CARGO_PKG_VERSION"),
        &stamp,
        response,
    );
    // The envelope `_analyse_json` writes, because a reader of either has one shape to parse
    // and the parity gate reads both through one function.
    serde_json::to_string(&serde_json::json!({ "ok": document }))
        .map_err(|error| MethodError::new_err(error.to_string()))
}

/// The engine's own cap, restated here because this signature offers it and a caller who
/// states nothing has to get the same sweep the other surfaces give them.
const DEFAULT_MAXIMUM_COMBINATIONS: usize = 512;

/// The dimensions to sweep: the steps whose rule varies, and the settings whose value does.
///
/// Both at once, which is the sweep the engine has always run and no surface could ask for.
/// Five onset rules by three values of `k` is `slot="onset", vary={"onset.k": [2, 5, 10]}`,
/// and the terminal writes the same request as `--slot onset --vary onset.k=2,5,10`. On
/// subject 01 trial 1 the six published values of `k` move a jump height 0.01981 m against
/// 0.01924 m for the five onset rules, so a reader asking about a number that rests on both
/// is asking one question rather than two.
fn axes_of(
    python: Python<'_>,
    slot: Option<&Bound<'_, PyAny>>,
    method_ids: Option<&Bound<'_, PyAny>>,
    vary: Option<&Bound<'_, PyAny>>,
) -> PyResult<Vec<spread::Axis>> {
    let named = match slot {
        Some(slot) => slots_named(slot)?,
        None => Vec::new(),
    };
    // One step is one axis. Written twice it was two, and the sweep squared its own
    // combinations: `slot=["onset", "onset"]` ran 25 of them for five rules, each combination
    // binding onset twice and the second binding winning, so the denominator every figure in
    // that document is reported over counted a set the caller never asked for. The terminal
    // refuses the repeat, in these words.
    for (position, name) in named.iter().enumerate() {
        if named[..position].contains(name) {
            return Err(MethodError::new_err(format!(
                "'{name}' is named twice, and one step is one axis"
            )));
        }
    }

    let listed = rules_named(method_ids, &named)?;
    let mut axes = Vec::new();
    for slot in &named {
        axes.push(axis_of(slot, listed.get(slot).cloned())?);
    }
    for (slot, ids) in &listed {
        if !named.contains(slot) {
            axes.push(axis_of(slot, Some(ids.clone()))?);
        }
    }
    axes.extend(settings_varied(python, vary)?);

    if axes.is_empty() {
        return Err(MethodError::new_err(
            "no step and no setting were named, so there is nothing to sweep".to_string(),
        ));
    }
    // One step and one setting is one axis, on the terms a step alone is one. The terminal
    // refuses the repeat in these words.
    for position in 0..axes.len() {
        let written = |axis: &spread::Axis| (axis.slot.clone(), axis.parameter.clone());
        if axes[..position]
            .iter()
            .any(|held| written(held) == written(&axes[position]))
        {
            let axis = &axes[position];
            let word = match axis.parameter.as_deref() {
                Some(parameter) => format!("{}.{parameter}", axis.slot),
                None => axis.slot.clone(),
            };
            return Err(MethodError::new_err(format!(
                "'{word}' is named twice, and one setting is one axis"
            )));
        }
    }
    Ok(axes)
}

/// The rules to compare, per step, where the caller named them rather than taking every rule
/// the build runs.
///
/// A plain list is the set for the one step named, which is the shape a folder run's
/// `--against` takes. Keyed by step it says which ids belong to which, so named rules on two
/// steps is one call: as a list that question cannot be asked at all, because the list cannot
/// say which step each id is for.
fn rules_named(
    method_ids: Option<&Bound<'_, PyAny>>,
    named: &[String],
) -> PyResult<BTreeMap<String, Vec<String>>> {
    let Some(method_ids) = method_ids else {
        return Ok(BTreeMap::new());
    };
    if let Ok(by_slot) = method_ids.extract::<BTreeMap<String, Vec<String>>>() {
        return Ok(by_slot);
    }
    let listed: Vec<String> = method_ids.extract().map_err(|_| {
        MethodError::new_err(
            "method_ids is a list of registry ids, or a mapping from a step to its ids".to_string(),
        )
    })?;
    match named {
        [only] => Ok(BTreeMap::from([(only.clone(), listed)])),
        _ => Err(MethodError::new_err(
            "a list of method_ids describes one step, so name one step or key the ids by step"
                .to_string(),
        )),
    }
}

/// One setting to sweep per key, written as the terminal writes it: `"onset.k"`, and
/// `"global.gravity_meters_per_second_squared"` for the value the run carries rather than a
/// rule.
///
/// Numbers or names per key and never both. A name is a choice in the sense a number is, so
/// `{"epoch_impulse.convention": ["net", "gross"]}` is a sweep, and an axis carrying one of
/// each has no width between them.
fn settings_varied(
    python: Python<'_>,
    vary: Option<&Bound<'_, PyAny>>,
) -> PyResult<Vec<spread::Axis>> {
    let Some(vary) = vary else {
        return Ok(Vec::new());
    };
    let stated: BTreeMap<String, Vec<Py<PyAny>>> = vary.extract().map_err(|_| {
        MethodError::new_err(
            "vary is keyed by the step and the setting, as vary={\"onset.k\": [2, 5, 10]}"
                .to_string(),
        )
    })?;

    let mut axes = Vec::new();
    for (qualified, alternatives) in stated {
        let Some((slot, parameter)) = qualified.split_once('.') else {
            return Err(MethodError::new_err(format!(
                "'{qualified}' names no step, and vary is keyed by the step and the setting"
            )));
        };
        if alternatives.is_empty() {
            return Err(MethodError::new_err(format!(
                "no value was named for {qualified}, so there is nothing to sweep"
            )));
        }

        let bound: Vec<Bound<'_, PyAny>> = alternatives
            .iter()
            .map(|value| value.bind(python).clone())
            .collect();
        let numbers: Option<Vec<f64>> = bound.iter().map(|value| value.extract().ok()).collect();
        let axis = match numbers {
            Some(values) => {
                for value in &values {
                    if !value.is_finite() {
                        return Err(MethodError::new_err(format!(
                            "{qualified} was given {value}, and a value to sweep is finite"
                        )));
                    }
                }
                distinct(&qualified, values.iter().map(|value| value.to_string()))?;
                spread::Axis {
                    slot: slot.to_string(),
                    parameter: Some(parameter.to_string()),
                    values,
                    ..Default::default()
                }
            }
            None => {
                let names: Option<Vec<String>> =
                    bound.iter().map(|value| value.extract().ok()).collect();
                let Some(options) = names else {
                    return Err(MethodError::new_err(format!(
                        "{qualified} was given numbers and names, and one axis compares one \
                         of them"
                    )));
                };
                distinct(&qualified, options.iter().cloned())?;
                spread::Axis {
                    slot: slot.to_string(),
                    parameter: Some(parameter.to_string()),
                    options,
                    ..Default::default()
                }
            }
        };
        axes.push(axis);
    }
    Ok(axes)
}

/// An alternative written twice is a variant paired with a copy of itself, which pulls the
/// spread toward a number no second choice produced while counting in the denominator. The
/// terminal refuses it in these words.
fn distinct(qualified: &str, written: impl Iterator<Item = String>) -> PyResult<()> {
    let mut seen: Vec<String> = Vec::new();
    for one in written {
        if seen.contains(&one) {
            return Err(MethodError::new_err(format!(
                "{qualified} names {one} twice, and one value is one variant"
            )));
        }
        seen.push(one);
    }
    Ok(())
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

/// A step named on its own is swept over the rules the binding table holds for it, and a step
/// the table holds one rule for has no alternative for that rule to be compared against.
///
/// The terminal refuses `--slot` there, in the sentence this raises. This surface let the name
/// through, and the engine then refused it as a name that is not one of the four axes a sweep
/// can vary, measured on all six of the steps the binding table holds one rule for. That is a
/// different fault from the one the caller has: it sends a reader looking for a typo or a
/// binding they forgot, and never says that the step is reached one way in this build. Two
/// keyboards, one question, two accounts of what went wrong.
///
/// A list of ids the caller wrote is the set they mean, one long or five, and is not held to
/// that floor: it is the shape a folder run's `--against` takes, where the bound rule named
/// against itself is one variant and runs. An empty list names nothing at all.
fn axis_of(slot: &str, method_ids: Option<Vec<String>>) -> PyResult<spread::Axis> {
    let ids = match method_ids {
        Some(named) => {
            if named.is_empty() {
                return Err(MethodError::new_err(format!(
                    "no rule was named for {slot}, so there is nothing to sweep"
                )));
            }
            named
        }
        None => {
            let table: Vec<String> = bindings_for(slot)
                .map(|binding| binding.id.to_string())
                .collect();
            if table.len() < 2 {
                let runs = match table.len() {
                    0 => "no rule",
                    _ => "one rule",
                };
                return Err(MethodError::new_err(format!(
                    "this analysis runs {runs} for {slot}, so there is nothing to sweep"
                )));
            }
            table
        }
    };
    Ok(spread::Axis {
        slot: slot.to_string(),
        parameter: None,
        values: Vec::new(),
        options: Vec::new(),
        method_ids: ids,
    })
}
