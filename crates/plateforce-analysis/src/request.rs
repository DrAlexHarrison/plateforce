//! What a caller asks for: a rule per slot, the values bound to it, and the constants the
//! request itself carries.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use plateforce_core::provenance::{ParameterSource, PresetAttribution};
use plateforce_core::{Refusal, STANDARD_GRAVITY_METERS_PER_SECOND_SQUARED};
use plateforce_registry::{Preset, PresetBinding, Registry};
use serde::{Deserialize, Serialize};

use crate::binding::{ONSET_CONSTRUCT, TAKEOFF_CONSTRUCT, WEIGHING_CONSTRUCT};

/// What the registry declares a rule falls back to for each name nobody states.
///
/// The engine takes bound values and knows nothing about where they came from, which is the
/// property that lets one binding layer serve four surfaces. So a published default reaches a
/// rule the way every other published value does: on the request, put there by the software
/// reading the registry rather than by a caller.
///
/// It used to reach a rule as a string in the rule's own body, which is a second home for a
/// value the registry publishes. Editing the registry alone, which is the act
/// "adding a method is a data edit" sanctions, then gave one build two answers:
/// `registry show bwepoch.fixed_window` reported `centre = median` while a run of that entry
/// bound `mean`, and the record published the second while a reader checked the first. Worse
/// than documentary, because the notebook surface already read `default_key` and the other
/// three did not: one published method returned two numbers, decided by which surface asked.
///
/// Not on the wire. A caller states what it chose; this is what the registry says, and a
/// field a caller could send would let a caller publish a default nobody wrote down.
/// Keyed by entry id rather than held per slot, because a sweep swaps the rule in a slot and
/// keeps everything else: `spread::materialise` writes a new `method_id` onto a cloned choice,
/// and a block read for the rule that was there before would answer for the rule that is there
/// now. Keyed by id, the lookup happens where the rule reads the value and cannot go stale.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DeclaredDefaults {
    by_entry: BTreeMap<String, EntryDefaults>,
}

/// What one entry declares, flattened with the operator entries a rule filed under its
/// construct composes.
///
/// The operators are folded in because a rule reads their names through its own binding:
/// `onset.threshold.noise_relative` reads `selection`, and the value that name falls back to
/// is declared on `onset.op.crossing_selection`, an entry no caller ever names. Splitting the
/// record back onto the operator's own row is `bound_with_operators`' job, and it happens
/// after the rule has read anything.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct EntryDefaults {
    /// Values the entry declares under `default_key`, which are choices between published
    /// names.
    names: BTreeMap<String, String>,
    /// Values the entry declares under `default`, which are quantities.
    numbers: BTreeMap<String, f64>,
}

impl EntryDefaults {
    /// The name this parameter falls back to, or nothing where the registry declares none.
    pub fn name(&self, parameter: &str) -> Option<&str> {
        self.names.get(parameter).map(String::as_str)
    }

    /// The quantity this parameter falls back to, or nothing where the registry declares none.
    pub fn number(&self, parameter: &str) -> Option<f64> {
        self.numbers.get(parameter).copied()
    }
}

impl DeclaredDefaults {
    /// Everything the registry declares, for every entry it carries.
    ///
    /// The whole registry rather than the entries one request names, so a sweep that swaps a
    /// rule into a slot finds that rule's declarations already here.
    pub fn of(registry: &Registry) -> Self {
        let mut by_entry: BTreeMap<String, EntryDefaults> = BTreeMap::new();
        for (id, entry) in &registry.methods {
            let mut declared = EntryDefaults::default();
            let composed = operators_composed_under(&entry.construct);
            for source in std::iter::once(id.as_str()).chain(composed.iter().copied()) {
                let Some(read) = registry.methods.get(source) else {
                    continue;
                };
                for parameter in &read.parameters {
                    if let Some(key) = &parameter.default_key {
                        declared.names.insert(parameter.name.clone(), key.clone());
                    }
                    if let Some(value) = parameter.default {
                        declared.numbers.insert(parameter.name.clone(), value);
                    }
                }
            }
            by_entry.insert(id.clone(), declared);
        }
        Self { by_entry }
    }

    /// What one entry declares. An entry this registry does not carry declares nothing, which
    /// leaves every name it reads unstated and refused rather than filled from a neighbour.
    ///
    /// The id is resolved through `records_under` first, because a caller may reach a rule by
    /// a compound name the registry spells as a pair, and the declarations belong to the entry
    /// a reader can look up rather than to the name they arrived by.
    pub fn of_entry(&self, method_id: &str) -> &EntryDefaults {
        let entry_id = crate::binding::records_under(method_id);
        static NOTHING: std::sync::LazyLock<EntryDefaults> =
            std::sync::LazyLock::new(EntryDefaults::default);
        self.by_entry.get(entry_id).unwrap_or(&NOTHING)
    }

    /// Whether anything was read into this at all, which is the difference between a request
    /// the software prepared against a registry and one built by hand.
    pub fn is_empty(&self) -> bool {
        self.by_entry.is_empty()
    }
}

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
    /// Set when the rule itself is the one the registry declares for a construct nobody
    /// named, or the one an interface pre-selected with nobody asked. Distinct from
    /// `method_from_recommendation`, which is an act somebody performed, exactly as
    /// `from_registry_default` below is distinct from `recommended`.
    #[serde(default)]
    pub method_from_registry_default: bool,
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
    /// What the registry declares, for every entry it carries.
    ///
    /// Skipped over the wire, and filled by `AnalysisRequest::reading` rather than by any
    /// caller: it is what the registry says, not what a caller chose. Shared rather than
    /// copied into each slot, because a request holds several choices and a sweep clones the
    /// whole request per combination.
    #[serde(skip)]
    pub declared: Arc<DeclaredDefaults>,
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
    /// Whether the rule itself is the registry's own, running because nobody named one.
    pub method_from_registry_default: bool,
}

impl Claims<'_> {
    /// How the rule itself was chosen, in the vocabulary and the precedence its values are
    /// recorded under.
    ///
    /// The same four claims `Resolution::stated_source` weighs for a value, beaten in the
    /// same order, because a reader asking who chose the rule is asking the question they
    /// ask of every number under it. A rule nobody named is `Assumed`, which is this
    /// vocabulary's word for the software's own choice rather than the reader's: the record
    /// used to spell it `Stated`, which put the reader's signature on 15 of the 18 rules a
    /// plain request runs.
    pub fn method_source(&self) -> ParameterSource {
        if self.preset.is_some() {
            ParameterSource::Cited
        } else if self.method_from_recommendation {
            ParameterSource::Recommended
        } else if self.method_from_registry_default {
            ParameterSource::Assumed
        } else {
            ParameterSource::Stated
        }
    }
}

/// `Default` is for the wire and for fixtures, where an absent field means the serde
/// default. The four surface builders spell every field without a rest pattern, on
/// purpose, so the next field this struct gains is a compile error at each surface rather
/// than a value defaulting silently into somebody's record.
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
    /// Set when the rule itself is the one the registry declares for a construct nobody
    /// named. See `MethodChoice`.
    #[serde(default)]
    pub method_from_registry_default: bool,
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
    /// What the registry declares, for every entry it carries.
    ///
    /// Skipped over the wire, and filled by `AnalysisRequest::reading` rather than by any
    /// caller: it is what the registry says, not what a caller chose. Shared rather than
    /// copied into each slot, because a request holds several choices and a sweep clones the
    /// whole request per combination.
    #[serde(skip)]
    pub declared: Arc<DeclaredDefaults>,
}

impl MethodChoice {
    pub fn claims(&self) -> Claims<'_> {
        Claims {
            recommended: &self.recommended,
            from_registry_default: &self.from_registry_default,
            cited: &self.cited,
            preset: self.preset.as_ref(),
            method_from_recommendation: self.method_from_recommendation,
            method_from_registry_default: self.method_from_registry_default,
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
            method_from_registry_default: self.method_from_registry_default,
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
    /// Where the number above came from, which the number alone cannot say.
    ///
    /// The field is filled by `Default` and by `serde` whether or not anybody chose a value,
    /// so a request holding standard gravity and a request whose author measured 9.80665 at
    /// their own plate are byte-identical without this. A rule that publishes its own default
    /// reads this to tell a value it must honour from one nobody stated, and gravity varies by
    /// half a percent across the Earth's surface, which is fifteen times the difference between
    /// the two constants the tools argue over.
    #[serde(default = "assumed")]
    pub gravity_source: ParameterSource,
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
    /// out is not the same as declining to condition: it is the software choosing.
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

/// What a request claims about a gravity nobody stated, which is the claim every request
/// starts from. A caller that chose the value says so by writing the field.
fn assumed() -> ParameterSource {
    ParameterSource::Assumed
}

/// For the wire and for fixtures, like `MethodChoice` and `WeighingChoice`. The surface
/// builders do not use it: each spells every field, so a field added here breaks each
/// surface at compile time instead of defaulting silently, the way `RegistryStamp`'s
/// consumers destructure without a rest pattern.
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
            gravity_source: assumed(),
            registry_backed_ids: Vec::new(),
            derived: BTreeMap::new(),
            conditioning: BTreeMap::new(),
            body_mass_kilograms: None,
        }
    }
}

/// The names this request binds for the whole analysis, as `bound_globals` reports them.
pub const GRAVITY_GLOBAL: &str = "gravity_meters_per_second_squared";
pub const BODY_MASS_GLOBAL: &str = "body_mass_kilograms";
pub const TOUCHDOWN_GLOBAL: &str = "touchdown_index";

/// A gravity somebody stated, or the constant nobody was asked about, with the claim that
/// tells the two apart.
///
/// One home for the pair, because the value and the claim are two fields and a surface that
/// writes one and leaves the other records a value its caller measured as one nobody was
/// asked about. Every surface that offers a gravity goes through here.
pub fn gravity_stated(value: Option<f64>) -> (f64, ParameterSource) {
    match value {
        Some(stated) => (stated, ParameterSource::Stated),
        None => (standard_gravity(), ParameterSource::Assumed),
    }
}

impl AnalysisRequest {
    pub(crate) fn is_backed(&self, method_id: &str) -> bool {
        self.registry_backed_ids.iter().any(|id| id == method_id)
    }

    /// Reads what the registry declares for every rule this request names, so the rules run
    /// on the published defaults rather than on copies of them.
    ///
    /// One home called by every surface, rather than each surface filling its own values in.
    /// The notebook surface already did it alone, which is how one build came to answer two
    /// numbers for one published method: it followed the registry and the terminal, R and the
    /// browser followed a string in the rule.
    ///
    /// Called after a pipeline has been adopted, because adopting one names rules this
    /// request did not name before, and a rule whose entry was never read falls back to
    /// nothing and refuses by name.
    pub fn reading(&mut self, registry: &Registry) {
        self.declared_from(Arc::new(DeclaredDefaults::of(registry)));
    }

    /// The same, for a surface that read the registry once and kept the answer.
    ///
    /// One writer either way, so no surface fills its own slots and none can fill some of
    /// them. A construct the request names and this misses is a rule reading nothing, which
    /// refuses rather than falling back.
    pub fn declared_from(&mut self, declared: Arc<DeclaredDefaults>) {
        self.weighing.declared = Arc::clone(&declared);
        self.onset.declared = Arc::clone(&declared);
        self.takeoff.declared = Arc::clone(&declared);
        for choice in self.derived.values_mut().chain(self.conditioning.values_mut()) {
            choice.declared = Arc::clone(&declared);
        }
    }

    /// Writes a gravity for the whole analysis, with the claim that says whether anybody
    /// chose it. `None` is the caller declining to state one.
    pub fn state_gravity(&mut self, value: Option<f64>) {
        let (value, source) = gravity_stated(value);
        self.gravity_meters_per_second_squared = value;
        self.gravity_source = source;
    }

    /// Every value this request binds for the whole analysis rather than for one rule.
    ///
    /// Destructured without a rest pattern, on the model `request_digest` already uses: a
    /// field added to this type stops this compiling rather than silently leaving the record
    /// blind to it. The two populations are different, and both are exhaustive by the same
    /// mechanism. A digest identifies a request; this reports what no rule's row can.
    ///
    /// A rule's choices are absent because a rule records its own, on its own row, under the
    /// names its registry entry declares. What lands here is the remainder: the values that
    /// belong to the analysis and to no entry in the registry.
    pub fn bound_globals(&self) -> Vec<crate::response::BoundGlobal> {
        // A field added to this type breaks this line, and that is the guard rather than a
        // bug: rule with a `_`, or give it a row below and let the record carry it.
        let AnalysisRequest {
            weighing: _,
            onset: _,
            takeoff: _,
            touchdown_index,
            gravity_meters_per_second_squared,
            gravity_source,
            registry_backed_ids: _,
            derived: _,
            conditioning: _,
            body_mass_kilograms,
        } = self;

        let mut bound = vec![crate::response::BoundGlobal::of(
            GRAVITY_GLOBAL,
            *gravity_meters_per_second_squared,
            "meters_per_second_squared",
            *gravity_source,
        )];
        // Absent where the caller stated nothing, because a row for a value nobody supplied
        // would report the software's silence as the caller's choice. A touchdown the caller
        // did not place is found by the takeoff threshold, and the rule that owns that
        // threshold is already on the record under its own id.
        if let Some(kilograms) = body_mass_kilograms {
            bound.push(crate::response::BoundGlobal::of(
                BODY_MASS_GLOBAL,
                *kilograms,
                "kilograms",
                ParameterSource::Stated,
            ));
        }
        if let Some(index) = touchdown_index {
            bound.push(crate::response::BoundGlobal::of(
                TOUCHDOWN_GLOBAL,
                *index as f64,
                "samples",
                ParameterSource::Stated,
            ));
        }
        bound
    }

    /// Lays a published pipeline's bindings onto this request.
    ///
    /// Every value the source states is written and marked as cited, so the record names the
    /// pipeline that chose it rather than reading as though the caller typed it. A value the
    /// caller stated under the same name stays and the pipeline's is recorded as superseded,
    /// so the result carries both numbers and which one ran.
    ///
    /// A construct the caller already named a rule for is refused rather than overwritten in
    /// either direction, so a pipeline's name is never reported over a rule that pipeline
    /// does not state.
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

/// The operator entries a rule filed under this construct composes.
///
/// Read off the engine's own lists rather than written out, for the reason
/// `fallbacks_match_the_registry` had to stop writing them out: a hand-written copy carried
/// six of the thirteen this build composes and nothing said so.
fn operators_composed_under(construct: &str) -> &'static [&'static str] {
    match construct {
        ONSET_CONSTRUCT => crate::slots::movement_onset::ONSET_OPERATOR_IDS,
        TAKEOFF_CONSTRUCT => crate::slots::takeoff::TAKEOFF_OPERATOR_IDS,
        _ => &[],
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

/// What every surface does before running, for a request one of this crate's own tests built
/// by hand.
///
/// One home for the three test modules that build requests, rather than a loader in each, and
/// read once: the declarations are the same registry's on every request, and the sweeps call
/// this several hundred times.
#[cfg(test)]
pub(crate) fn prepared(mut request: AnalysisRequest) -> AnalysisRequest {
    static DECLARED: std::sync::LazyLock<Arc<DeclaredDefaults>> = std::sync::LazyLock::new(|| {
        let registry = Registry::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../registry"))
            .expect("the committed registry loads");
        Arc::new(DeclaredDefaults::of(&registry))
    });
    request.declared_from(Arc::clone(&DECLARED));
    request
}
