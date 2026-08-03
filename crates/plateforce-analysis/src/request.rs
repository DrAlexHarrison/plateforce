//! What a caller asks for: a rule per slot, the values bound to it, and the constants the
//! request itself carries.

use std::collections::{BTreeMap, BTreeSet};

use plateforce_core::provenance::PresetAttribution;
use plateforce_core::{Refusal, STANDARD_GRAVITY_METERS_PER_SECOND_SQUARED};
use plateforce_registry::{Preset, PresetBinding, Registry};
use serde::{Deserialize, Serialize};

use crate::binding::{ONSET_CONSTRUCT, TAKEOFF_CONSTRUCT, WEIGHING_CONSTRUCT};

/// Unknown fields are refused rather than ignored. A caller whose field name has drifted
/// from this one would otherwise send every value it holds into nothing, and each rule
/// would run its own value under the id the caller asked for.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MethodChoice {
    /// Empty when nothing has named a rule for this construct yet, which is what a request
    /// naming a published pipeline sends before the pipeline is laid onto it. An empty id
    /// reaches no rule and is refused by name rather than resolving to a neighbour.
    #[serde(default)]
    pub method_id: String,
    #[serde(default)]
    pub parameters: BTreeMap<String, f64>,
    /// Enumerated settings that are not numbers, such as which divisor a standard
    /// deviation uses. They are choices in exactly the same sense and are fingerprinted
    /// the same way.
    #[serde(default)]
    pub options: BTreeMap<String, String>,
    /// Set when the user dragged the marker. An override is a provenance fact, not a
    /// bypass, so it is reported next to the number it changed.
    #[serde(default)]
    pub manual_index: Option<usize>,
    /// Names in `parameters` and `options` whose value came from the caller accepting the
    /// registry's recommendation rather than choosing it. An acceptance and a hand-typed
    /// value produce byte-identical requests otherwise, so the request has to say which.
    #[serde(default)]
    pub recommended: BTreeSet<String>,
    /// Set when the rule itself came from the recommendation rather than being picked.
    #[serde(default)]
    pub method_from_recommendation: bool,
    /// Names the caller filled from the registry's default with nobody asked. Distinct from
    /// `recommended`, which is an act somebody performed.
    #[serde(default)]
    pub from_registry_default: BTreeSet<String>,
    /// Names whose value a published pipeline the caller adopted supplied. The caller chose
    /// the pipeline by its id and its citation, and a named author stands behind the value,
    /// which is neither of the two claims above.
    #[serde(default)]
    pub cited: BTreeSet<String>,
    /// The pipeline this rule and its cited values were adopted from.
    #[serde(default)]
    pub preset: Option<PresetAttribution>,
}

/// What a choice claims about where its values came from, borrowed together.
///
/// One argument rather than four, so a rule reaching for the values cannot reach them
/// without the claims, and a slot added later cannot compile while recording a published
/// author's number as one the reader typed.
pub struct Claims<'a> {
    pub recommended: &'a BTreeSet<String>,
    pub from_registry_default: &'a BTreeSet<String>,
    pub cited: &'a BTreeSet<String>,
    pub preset: Option<&'a PresetAttribution>,
    /// Whether the rule itself was accepted from the registry's recommendation rather than
    /// picked. A bulk acceptance and a considered pick move the number identically.
    pub method_from_recommendation: bool,
}

/// `Default` so a caller can build one with `..Default::default()`. The next field this
/// struct gains would otherwise break every exhaustive literal in the workspace at once.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WeighingChoice {
    /// Empty when nothing has named a rule for this construct yet. See `MethodChoice`.
    #[serde(default)]
    pub method_id: String,
    #[serde(default)]
    pub start_index: Option<usize>,
    /// Named as the registry names them. The three weighing rules each carry their own name
    /// for the window's length, so there is no one field that would fit all three.
    #[serde(default)]
    pub parameters: BTreeMap<String, f64>,
    #[serde(default)]
    pub options: BTreeMap<String, String>,
    /// Names in `parameters` and `options` whose value came from the caller accepting the
    /// registry's recommendation rather than choosing it. An acceptance and a hand-typed
    /// value produce byte-identical requests otherwise, so the request has to say which.
    #[serde(default)]
    pub recommended: BTreeSet<String>,
    /// Set when the rule itself came from the recommendation rather than being picked.
    #[serde(default)]
    pub method_from_recommendation: bool,
    /// Names the caller filled from the registry's default with nobody asked. Distinct from
    /// `recommended`, which is an act somebody performed.
    #[serde(default)]
    pub from_registry_default: BTreeSet<String>,
    /// Names whose value a published pipeline the caller adopted supplied. The caller chose
    /// the pipeline by its id and its citation, and a named author stands behind the value,
    /// which is neither of the two claims above.
    #[serde(default)]
    pub cited: BTreeSet<String>,
    /// The pipeline this rule and its cited values were adopted from.
    #[serde(default)]
    pub preset: Option<PresetAttribution>,
}

impl MethodChoice {
    pub fn claims(&self) -> Claims<'_> {
        Claims {
            recommended: &self.recommended,
            from_registry_default: &self.from_registry_default,
            cited: &self.cited,
            preset: self.preset.as_ref(),
            method_from_recommendation: self.method_from_recommendation,
        }
    }
}

impl WeighingChoice {
    pub fn claims(&self) -> Claims<'_> {
        Claims {
            recommended: &self.recommended,
            from_registry_default: &self.from_registry_default,
            cited: &self.cited,
            preset: self.preset.as_ref(),
            method_from_recommendation: self.method_from_recommendation,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AnalysisRequest {
    pub weighing: WeighingChoice,
    pub onset: MethodChoice,
    pub takeoff: MethodChoice,
    #[serde(default)]
    pub touchdown_index: Option<usize>,
    /// Gravity is a bound parameter because the tools disagree on it, and the core takes
    /// it as an argument for the same reason.
    #[serde(default = "standard_gravity")]
    pub gravity_meters_per_second_squared: f64,
    #[serde(default)]
    pub registry_backed_ids: Vec<String>,
    /// A rule chosen for a construct computed from the resolved landmarks, keyed by the
    /// construct id the registry declares, not by a slot word.
    ///
    /// Defaulted, so every writer that predates it keeps parsing against
    /// `deny_unknown_fields`. That hazard runs the other way: a writer emitting a key the
    /// reader lacks. So the field lands before anything emits it, never the reverse.
    #[serde(default)]
    pub derived: BTreeMap<String, MethodChoice>,
    /// A rule chosen for a construct that conditions the signal, keyed by the construct id
    /// the registry declares. Runs before the landmark rules, because the signal they read
    /// is the one these produce.
    ///
    /// A construct this build runs and the request does not name still runs, under the rule
    /// declared as its default, and that rule is on the record like any other. Leaving it
    /// out is not the same as declining to condition: it is the software choosing, and a
    /// choice nobody can read is the defect this field exists to close.
    #[serde(default)]
    pub conditioning: BTreeMap<String, MethodChoice>,
    /// The athlete's mass, which is a different quantity from the weighed system mass:
    /// system weight includes any bar and bodyweight does not.
    ///
    /// Three level-one entries divide by it and none can take the weighed mass instead. A
    /// request that states none leaves them declining by name, which is the whole of what
    /// this field buys over substituting the number next to it.
    #[serde(default)]
    pub body_mass_kilograms: Option<f64>,
}

fn standard_gravity() -> f64 {
    STANDARD_GRAVITY_METERS_PER_SECOND_SQUARED
}

/// So a caller can build one with `..Default::default()`, which is what `MethodChoice` and
/// `WeighingChoice` already offer and for the same reason: the next field this struct gains
/// would otherwise break every exhaustive literal in the workspace at once.
///
/// Gravity defaults to the same constant `serde` fills, rather than to zero. A request whose
/// method ids are empty is refused by name, so nothing here resolves to a rule by accident.
impl Default for AnalysisRequest {
    fn default() -> Self {
        Self {
            weighing: WeighingChoice::default(),
            onset: MethodChoice::default(),
            takeoff: MethodChoice::default(),
            touchdown_index: None,
            gravity_meters_per_second_squared: standard_gravity(),
            registry_backed_ids: Vec::new(),
            derived: BTreeMap::new(),
            conditioning: BTreeMap::new(),
            body_mass_kilograms: None,
        }
    }
}

impl AnalysisRequest {
    pub(crate) fn is_backed(&self, method_id: &str) -> bool {
        self.registry_backed_ids.iter().any(|id| id == method_id)
    }

    /// Lays a published pipeline's bindings onto this request.
    ///
    /// Every value the source states is written and marked as cited, so the record names the
    /// pipeline that chose it rather than reading as though the caller typed it. A value the
    /// caller stated under the same name stays and the pipeline's is recorded as superseded,
    /// so the result carries both numbers and which one ran.
    ///
    /// A construct the caller already named a rule for is refused rather than overwritten in
    /// either direction. Reporting a pipeline's name over a rule that pipeline does not
    /// state is the defect this project documents in a competitor.
    ///
    /// A slot the source is silent about is left to the software's normal resolution and is
    /// never attributed here, which is a fact about the source rather than about this build.
    pub fn adopt(&mut self, preset: &Preset) -> Result<(), Box<Refusal>> {
        for binding in &preset.bindings {
            let runnable: Vec<String> = crate::binding::bindings_for_construct(&binding.construct)
                .map(|binding| binding.id.to_string())
                .collect();
            if !runnable.contains(&binding.method_id) {
                return Err(Box::new(Refusal::method_not_implemented(
                    binding.method_id.clone(),
                    binding.construct.clone(),
                    runnable,
                )));
            }

            let target = match binding.construct.as_str() {
                WEIGHING_CONSTRUCT => {
                    let slot = &mut self.weighing;
                    Slot {
                        method_id: &mut slot.method_id,
                        parameters: &mut slot.parameters,
                        options: &mut slot.options,
                        cited: &mut slot.cited,
                        attributed: &mut slot.preset,
                    }
                }
                ONSET_CONSTRUCT => slot_of(&mut self.onset),
                TAKEOFF_CONSTRUCT => slot_of(&mut self.takeoff),
                other => slot_of(self.derived.entry(other.to_string()).or_default()),
            };
            bind(target, &preset.id, binding)?;

            if !self.registry_backed_ids.contains(&binding.method_id) {
                self.registry_backed_ids.push(binding.method_id.clone());
            }
        }
        Ok(())
    }
}

/// The parts of a choice a pipeline writes into, borrowed together so one routine binds
/// every construct and the weighing slot cannot drift from the other two.
struct Slot<'a> {
    method_id: &'a mut String,
    parameters: &'a mut BTreeMap<String, f64>,
    options: &'a mut BTreeMap<String, String>,
    cited: &'a mut BTreeSet<String>,
    attributed: &'a mut Option<PresetAttribution>,
}

fn slot_of(choice: &mut MethodChoice) -> Slot<'_> {
    Slot {
        method_id: &mut choice.method_id,
        parameters: &mut choice.parameters,
        options: &mut choice.options,
        cited: &mut choice.cited,
        attributed: &mut choice.preset,
    }
}

/// One binding written into one slot, with what the caller had already stated kept and
/// recorded as having displaced the source's value.
fn bind(slot: Slot<'_>, preset_id: &str, binding: &PresetBinding) -> Result<(), Box<Refusal>> {
    if !slot.method_id.is_empty() && *slot.method_id != binding.method_id {
        return Err(Box::new(Refusal::name_not_accepted(
            preset_id,
            binding.construct.clone(),
            slot.method_id.clone(),
            vec![binding.method_id.clone()],
        )));
    }
    *slot.method_id = binding.method_id.clone();

    let mut attribution = PresetAttribution::of(preset_id);
    for (name, published) in &binding.parameters {
        match slot.parameters.get(name) {
            Some(_) => {
                attribution
                    .superseded_parameters
                    .insert(name.clone(), *published);
            }
            None => {
                slot.parameters.insert(name.clone(), *published);
                slot.cited.insert(name.clone());
            }
        }
    }
    for (name, published) in &binding.options {
        match slot.options.get(name) {
            Some(_) => {
                attribution
                    .superseded_options
                    .insert(name.clone(), published.clone());
            }
            None => {
                slot.options.insert(name.clone(), published.clone());
                slot.cited.insert(name.clone());
            }
        }
    }
    *slot.attributed = Some(attribution);
    Ok(())
}

/// The pipeline a caller named, or a refusal listing the ones this registry carries.
///
/// One lookup for every surface, so the sentence a caller reads for a name that is not a
/// pipeline is the same in a terminal, a browser tab, a traceback and an R condition.
pub fn preset_named<'a>(registry: &'a Registry, id: &str) -> Result<&'a Preset, Box<Refusal>> {
    registry.presets.get(id).ok_or_else(|| {
        Box::new(Refusal::preset_not_shipped(
            id,
            registry.presets.keys().cloned().collect(),
        ))
    })
}
