//! What one rule read while it ran, and where each value came from.
//!
//! Built from the request instead, a fingerprint omits every value the rule chose for
//! itself, which is a silent default.

use std::collections::{BTreeMap, BTreeSet};

use plateforce_core::provenance::{ParameterSource, PresetAttribution};
use plateforce_core::takeoff::ResidualComparison;
use plateforce_core::DispersionEstimator;
use serde::Serialize;

use crate::request::Claims;

pub(crate) struct Resolution<'a> {
    parameters: &'a BTreeMap<String, f64>,
    options: &'a BTreeMap<String, String>,
    /// Names the caller says it accepted from the registry's recommendation, and names it
    /// filled from a default with nobody asked. A rule cannot tell either from the number.
    recommended: &'a BTreeSet<String>,
    from_registry_default: &'a BTreeSet<String>,
    /// Names a published pipeline the caller adopted supplied. Disjoint from the two above
    /// by construction: a value the caller stated for itself leaves this set.
    cited: &'a BTreeSet<String>,
    /// The pipeline this rule was adopted from, travelling with the values it bound.
    adopted: Option<&'a PresetAttribution>,
    /// How the rule itself was chosen, settled from the claims before it read a value.
    method_source: ParameterSource,
    read: Vec<(String, String)>,
    sources: BTreeMap<String, ParameterSource>,
    consulted: BTreeSet<String>,
    /// The value behind each name a rule read as a number, kept as the number. A caller
    /// that wanted it back would otherwise parse the display text, which is a second
    /// derivation of a value this already holds.
    numbers: BTreeMap<String, f64>,
}

/// What one rule read, ready to become the record that travels with its answer. Public
/// because every rule computed from the landmarks hands one back, and that shape is the
/// contract for writing one.
#[derive(Debug, Clone)]
pub struct BoundValues {
    pub parameters: Vec<(String, String)>,
    pub sources: BTreeMap<String, ParameterSource>,
    pub unread: Vec<String>,
    pub numbers: BTreeMap<String, f64>,
    /// The published pipeline this rule was adopted from, carried alongside the values so a
    /// record cannot name the values without naming what chose them.
    pub preset: Option<PresetAttribution>,
    /// How the rule itself was chosen: named by the caller, accepted from the registry's
    /// recommendation, adopted with a published pipeline, or run because nobody named one.
    pub method_source: ParameterSource,
}

/// A reading nobody asked for is a rule nobody named, so the empty one starts at `Assumed`.
///
/// Written out rather than derived, because `ParameterSource` has no default and must not
/// gain one: a vocabulary whose strongest claim is what a forgotten field fills in is the
/// defect this type exists to record.
impl Default for BoundValues {
    fn default() -> Self {
        Self {
            parameters: Vec::new(),
            sources: BTreeMap::new(),
            unread: Vec::new(),
            numbers: BTreeMap::new(),
            preset: None,
            method_source: ParameterSource::Assumed,
        }
    }
}

impl<'a> Resolution<'a> {
    /// The values a rule reads and, in one argument, everything the choice claims about
    /// where they came from. Taken together on purpose: a rule handed the values without the
    /// claims records a published author's number as one the reader typed.
    pub(crate) fn over(
        parameters: &'a BTreeMap<String, f64>,
        options: &'a BTreeMap<String, String>,
        claims: Claims<'a>,
    ) -> Self {
        Self {
            parameters,
            options,
            recommended: claims.recommended,
            from_registry_default: claims.from_registry_default,
            cited: claims.cited,
            method_source: claims.method_source(),
            adopted: claims.preset,
            read: Vec::new(),
            sources: BTreeMap::new(),
            consulted: BTreeSet::new(),
            numbers: BTreeMap::new(),
        }
    }

    /// What the caller claimed about a name it did supply.
    ///
    /// A value present in the request is `stated` unless the caller said otherwise, because
    /// only the caller knows whether a number was typed, accepted in bulk, filled by an
    /// interface on the reader's behalf, or adopted with a published pipeline.
    fn stated_source(&self, name: &str) -> ParameterSource {
        if self.cited.contains(name) {
            ParameterSource::Cited
        } else if self.recommended.contains(name) {
            ParameterSource::Recommended
        } else if self.from_registry_default.contains(name) {
            ParameterSource::Assumed
        } else {
            ParameterSource::Stated
        }
    }

    /// Whether a recorded name carries the reader's own signature. Presence in the request
    /// is not the answer: an interface that fills a default sends the name too, marked, and
    /// reading presence as statedness put the reader's word on choices nobody made.
    pub(crate) fn recorded_as_stated(&self, name: &str) -> bool {
        matches!(self.sources.get(name), Some(ParameterSource::Stated))
    }

    pub(crate) fn record(&mut self, name: &str, value: String, source: ParameterSource) {
        self.read.push((name.to_string(), value));
        self.sources.insert(name.to_string(), source);
    }

    /// A name whose value is a quantity, recorded both as the text a reader sees and as
    /// the number the rule ran on.
    pub(crate) fn record_measured(
        &mut self,
        name: &str,
        value: f64,
        shown: String,
        source: ParameterSource,
    ) {
        self.numbers.insert(name.to_string(), value);
        self.record(name, shown, source);
    }

    /// The request's value for this name, and a note that the rule asked either way.
    pub(crate) fn stated(&mut self, name: &str) -> Option<f64> {
        self.consulted.insert(name.to_string());
        self.parameters.get(name).copied()
    }

    /// The request's value for a named choice, and a note that the rule asked either way.
    ///
    /// A rule whose entry marks a name required and publishes no default for it reads this and
    /// declines when it is absent. `option` cannot express that: it takes a fallback, and a
    /// fallback where the registry declares none is a decision the rule made for the caller.
    pub(crate) fn stated_name(&mut self, name: &str) -> Option<String> {
        self.consulted.insert(name.to_string());
        self.options.get(name).cloned()
    }

    pub(crate) fn number(&mut self, name: &str, fallback: f64) -> f64 {
        let stated = self.stated(name);
        let value = stated.unwrap_or(fallback);
        let source = match stated {
            Some(_) => self.stated_source(name),
            None => ParameterSource::Assumed,
        };
        self.record_measured(name, value, format_number(value), source);
        value
    }

    /// A value this rule's entry publishes a default for, which the caller may also have
    /// chosen once for the whole analysis rather than on this rule.
    ///
    /// Three claims, and they beat each other in that order. A value on this rule is the caller
    /// answering this entry's own question. A value chosen for the analysis is the caller
    /// answering it everywhere, and discarding it would run the number on a published constant
    /// while the record showed the caller's. Neither offered, the entry's default stands and is
    /// recorded as assumed, which is the whole of what `number` does.
    ///
    /// Kept apart from `number` because the middle claim is what `number` cannot express: it
    /// takes one fallback and cannot say that the fallback was somebody's choice.
    pub(crate) fn number_or_chosen(
        &mut self,
        name: &str,
        chosen_for_the_analysis: Option<(f64, ParameterSource)>,
        published_default: f64,
    ) -> f64 {
        if let Some(stated) = self.stated(name) {
            let source = self.stated_source(name);
            self.record_measured(name, stated, format_number(stated), source);
            return stated;
        }
        let (value, source) =
            chosen_for_the_analysis.unwrap_or((published_default, ParameterSource::Assumed));
        self.record_measured(name, value, format_number(value), source);
        value
    }

    pub(crate) fn seconds_as_samples(
        &mut self,
        name: &str,
        fallback_seconds: f64,
        sample_rate_hz: f64,
    ) -> usize {
        (self.number(name, fallback_seconds) * sample_rate_hz)
            .round()
            .max(0.0) as usize
    }

    /// The registry states every persistence and offset span in milliseconds while the core
    /// counts samples, so a span read under a registry name is converted here and nowhere
    /// else.
    pub(crate) fn milliseconds_as_samples(
        &mut self,
        name: &str,
        fallback_milliseconds: f64,
        sample_rate_hz: f64,
    ) -> usize {
        (self.number(name, fallback_milliseconds) / 1000.0 * sample_rate_hz)
            .round()
            .max(0.0) as usize
    }

    pub(crate) fn option(&mut self, name: &str, fallback: &'static str) -> String {
        self.consulted.insert(name.to_string());
        let stated = self.options.get(name).cloned();
        let source = match &stated {
            Some(_) => self.stated_source(name),
            None => ParameterSource::Assumed,
        };
        let value = stated.unwrap_or_else(|| fallback.to_string());
        self.record(name, value.clone(), source);
        value
    }

    /// An enumerated choice, refused rather than mapped onto a default when the value is
    /// not one this rule takes. Substituting quietly would record the word the caller wrote
    /// beside a number a different rule produced.
    pub(crate) fn enumerated<T: Copy>(
        &mut self,
        name: &str,
        fallback: &'static str,
        accepted: &[(&'static str, T)],
    ) -> Result<T, RuleRefusal> {
        let chosen = self.option(name, fallback);
        accepted
            .iter()
            .find(|(label, _)| *label == chosen)
            .map(|(_, value)| *value)
            .ok_or_else(|| {
                let offered: Vec<String> = accepted
                    .iter()
                    .map(|(label, _)| (*label).to_string())
                    .collect();
                // The id is left empty because a `Resolution` reads a request rather than
                // knowing which entry reached it. The boundary stamps the bound id on.
                RuleRefusal::Refused(Box::new(plateforce_core::Refusal::name_not_accepted(
                    "", name, chosen, offered,
                )))
            })
    }

    /// The same, for a name its entry states required with no default.
    ///
    /// Absent, it is refused rather than filled from the value a neighbouring entry publishes.
    /// The braking-start default rests on a 243-trial measurement made at the braking start,
    /// and that measurement says nothing about which signal is steadier at any other boundary,
    /// so borrowing it would carry a measurement onto a boundary it never touched.
    pub(crate) fn required_enumerated<T: Copy>(
        &mut self,
        method_id: &str,
        name: &str,
        accepted: &[(&'static str, T)],
    ) -> Result<T, RuleRefusal> {
        self.consulted.insert(name.to_string());
        let Some(chosen) = self.options.get(name).cloned() else {
            return Err(RuleRefusal::Refused(Box::new(
                plateforce_core::Refusal::required_parameter_unstated(method_id, name),
            )));
        };
        let source = self.stated_source(name);
        self.record(name, chosen.clone(), source);
        accepted
            .iter()
            .find(|(label, _)| *label == chosen)
            .map(|(_, value)| *value)
            .ok_or_else(|| {
                let offered: Vec<String> = accepted
                    .iter()
                    .map(|(label, _)| (*label).to_string())
                    .collect();
                RuleRefusal::Refused(Box::new(plateforce_core::Refusal::name_not_accepted(
                    method_id, name, chosen, offered,
                )))
            })
    }

    /// A value the choice of rule settles, which a caller may state only in agreement.
    ///
    /// Some operators are not free on every rule that composes them. `onset.op.search_upper_bound`
    /// publishes four landmarks to search back from and `onset.threshold.last_within_band`
    /// implements one, because searching back from a different landmark is a different rule.
    ///
    /// Refused rather than dropped, and the refusal names the operator that owns the choice
    /// rather than the rule that composed it, because the accepted values are the operator's.
    ///
    /// The source is the caller's claim where the caller stated the value, so a published
    /// pipeline that supplied it is still credited with it. Unstated it is `Assumed`, which is
    /// this vocabulary's word for the rule's own value rather than the reader's.
    ///
    /// `Stated` would be the other reading, that picking the rule picks the value as surely as
    /// typing it. `a_value_stated_for_a_folder_is_recorded_as_stated` holds the opposite: a run
    /// with no caller input contains no `stated` record at all. Recording an entailed value as
    /// `Stated` puts the reader's signature on 14 values in a record they did not touch.
    pub(crate) fn entailed(
        &mut self,
        operator_id: &str,
        name: &str,
        value: &'static str,
    ) -> Result<(), RuleRefusal> {
        self.entailed_from(operator_id, name, value, ParameterSource::Assumed)
    }

    /// A name an entry publishes that the rule behind it runs without.
    ///
    /// `entailed` fits a choice between names and this does not: there is no value to record,
    /// because the rule does the thing the name would bound and does it unbounded. Consulted
    /// either way, so silence is silence rather than a name nobody asked about, and refused
    /// where the caller wrote one, because a bound stated and dropped leaves the rule running
    /// unbounded while the record shows a reader who asked otherwise.
    ///
    /// `reads` is the denominator the sentence quotes, so a caller sees what this operator
    /// does take rather than only what it declines.
    pub(crate) fn runs_without(
        &mut self,
        operator_id: &str,
        name: &str,
        reads: &[&str],
    ) -> Result<(), RuleRefusal> {
        self.consulted.insert(name.to_string());
        if !self.parameters.contains_key(name) && !self.options.contains_key(name) {
            return Ok(());
        }
        Err(RuleRefusal::Refused(Box::new(
            plateforce_core::Refusal::unknown_parameter(
                operator_id,
                name,
                reads.iter().map(|read| (*read).to_string()).collect(),
            ),
        )))
    }

    /// The same, for a value another rule already settled and this one runs on.
    ///
    /// `entailed` records `Assumed` where the caller said nothing, which is the right claim for
    /// a value this rule's own choice fixes. A spread an onset band is scaled by is not that:
    /// the weighing rule computed it, and whether a reader picked the divisor is a fact about
    /// that rule's row. Carrying the claim across keeps the two rows saying one thing about one
    /// act, rather than the onset row reporting the reader's divisor as the software's.
    pub(crate) fn entailed_from(
        &mut self,
        operator_id: &str,
        name: &str,
        value: &str,
        unstated_source: ParameterSource,
    ) -> Result<(), RuleRefusal> {
        self.consulted.insert(name.to_string());
        let source = match self.options.get(name) {
            Some(chosen) if chosen != value => {
                return Err(RuleRefusal::Refused(Box::new(
                    plateforce_core::Refusal::name_not_accepted(
                        operator_id,
                        name,
                        chosen.clone(),
                        vec![value.to_string()],
                    ),
                )))
            }
            Some(_) => self.stated_source(name),
            None => unstated_source,
        };
        self.record(name, value.to_string(), source);
        Ok(())
    }

    pub(crate) fn dispersion(&mut self) -> Result<DispersionEstimator, RuleRefusal> {
        self.enumerated(
            "dispersion",
            "sample",
            &[
                ("population", DispersionEstimator::Population),
                ("sample", DispersionEstimator::Sample),
            ],
        )
    }

    pub(crate) fn residual_comparison(&mut self) -> Result<ResidualComparison, RuleRefusal> {
        self.enumerated(
            "comparison",
            "signed",
            &[
                ("signed", ResidualComparison::SignedValue),
                ("magnitude", ResidualComparison::Magnitude),
            ],
        )
    }

    /// Sorted, so the same binding fingerprints the same however the request was ordered.
    pub(crate) fn finish(mut self) -> BoundValues {
        let unread = self
            .parameters
            .keys()
            .chain(self.options.keys())
            .filter(|name| !self.consulted.contains(*name))
            .cloned()
            .collect();
        self.read.sort();
        BoundValues {
            parameters: self.read,
            sources: self.sources,
            unread,
            numbers: self.numbers,
            preset: self.adopted.cloned(),
            method_source: self.method_source,
        }
    }
}

/// The share of a pipeline's attribution that belongs to one row of a composition.
///
/// A binding names one composed rule and the composition splits it into the threshold rule
/// and the operators the registry files separately. The attribution follows the values, so an
/// operator running a value the pipeline never published is not reported under its author's
/// name, and a value the reader replaced is recorded as displaced on the row that ran it.
pub(crate) fn attribution_for(
    values: &BoundValues,
    adopted: Option<&PresetAttribution>,
    names_the_rule: bool,
) -> Option<PresetAttribution> {
    let adopted = adopted?;
    let holds = |name: &String| values.parameters.iter().any(|(held, _)| held == name);
    let mut share = PresetAttribution::of(&adopted.id);
    share.superseded_parameters = adopted
        .superseded_parameters
        .iter()
        .filter(|(name, _)| holds(name))
        .map(|(name, value)| (name.clone(), *value))
        .collect();
    share.superseded_options = adopted
        .superseded_options
        .iter()
        .filter(|(name, _)| holds(name))
        .map(|(name, value)| (name.clone(), value.clone()))
        .collect();

    let supplied_a_value = values
        .sources
        .values()
        .any(|source| *source == ParameterSource::Cited);
    if names_the_rule || supplied_a_value || share.was_overridden() {
        return Some(share);
    }
    None
}

pub(crate) fn format_number(value: f64) -> String {
    if value.fract().abs() < 1e-9 {
        format!("{value:.0}")
    } else {
        format!("{value}")
    }
}

pub(crate) fn dispersion_label(dispersion: DispersionEstimator) -> &'static str {
    dispersion.as_published_str()
}

/// Why a rule produced nothing.
///
/// The sentence goes to `warnings` for somebody reading the trace. Both variants carry the
/// fields a caller branches on, so no rule reaches a surface with a sentence and nothing
/// else.
#[derive(Debug, Clone)]
pub enum RuleRefusal {
    /// The core's own error, which already carries its code and its fields.
    Trial(plateforce_core::TrialError),
    /// A refusal the rule built itself, under the code it decided it was declining on.
    /// A rule that leaves `method_id` empty is stamped with the id it was bound to at the
    /// boundary, because a core operator does not know which entry a caller reached it by.
    /// Boxed because a `Refusal` is 192 bytes against this enum's other variant at 72,
    /// and every rule in the tree returns a `Result` carrying it on the error side.
    Refused(Box<plateforce_core::Refusal>),
}

impl std::fmt::Display for RuleRefusal {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RuleRefusal::Trial(error) => write!(formatter, "{error}"),
            RuleRefusal::Refused(refusal) => formatter.write_str(refusal.message()),
        }
    }
}

/// The one place either variant becomes the record every surface publishes.
///
/// Neither arm decides a code: `TrialError` carries its own and a rule that built a
/// `Refusal` chose the one it was declining under. Written once because a rule that
/// declines reaches a caller through several paths, and a second copy of this match would be
/// free to answer one of them differently.
impl From<RuleRefusal> for plateforce_core::Refusal {
    fn from(refusal: RuleRefusal) -> Self {
        match refusal {
            RuleRefusal::Trial(error) => plateforce_core::Refusal::from(error),
            RuleRefusal::Refused(refused) => *refused,
        }
    }
}

/// One rule that declined, with everything the rule itself could not know: which construct
/// it was filling and which id a caller reached it by.
///
/// Carried on the rule rather than recovered at each surface from the shape of the method id.
#[derive(Debug, Clone)]
pub struct DeclinedRule {
    /// Named as the registry names constructs, so a caller can look it up.
    pub construct: &'static str,
    pub method_id: String,
    pub refusal: RuleRefusal,
}

/// On the wire it is the record a caller branches on, which is what every other surface
/// already builds from it.
///
/// Written here rather than left to each reader: a refusal that cannot cross is a result
/// without its method.
impl Serialize for DeclinedRule {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        crate::document::refusal_from_rule(self).serialize(serializer)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct BoundMethod {
    pub method_id: String,
    pub bound_parameters: Vec<(String, String)>,
    /// Where each name in `bound_parameters` came from. A value the caller typed and one
    /// the rule fell back to move the number identically, so the record keeps them apart.
    pub parameter_sources: BTreeMap<String, ParameterSource>,
    /// Names the request carried that this rule does not read.
    pub unread_parameters: Vec<String>,
    pub registry_backed: bool,
    pub manual_override: bool,
    /// The published pipeline this rule and its cited values were adopted from. A surface
    /// that printed the values without this would report a published author's numbers as
    /// though the reader had picked them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preset: Option<PresetAttribution>,
    /// How the rule itself was chosen, in the vocabulary its values are recorded under. A
    /// bulk acceptance, a considered pick, a pipeline's binding and a rule nobody named are
    /// four records, not one, and the last of them is the one this software exists to stop
    /// wearing the reader's signature.
    pub method_source: ParameterSource,
    /// The names in `bound_parameters` the rule read as quantities, with their values.
    /// Skipped over the wire, where every value is already the text beside its name.
    #[serde(skip)]
    pub numeric_values: BTreeMap<String, f64>,
}

impl BoundMethod {
    /// What this rule was bound to, split the way a fingerprint carries it: quantities
    /// against choices between named alternatives.
    pub fn quantities(&self) -> Vec<(String, f64)> {
        self.bound_parameters
            .iter()
            .filter_map(|(name, _)| {
                self.numeric_values
                    .get(name)
                    .map(|value| (name.clone(), *value))
            })
            .collect()
    }

    /// The record this rule leaves behind, with each value carrying where it came from.
    ///
    /// A value the caller stated and one the rule fell back to move the number identically,
    /// so the distinction has to survive into the record rather than living only here.
    pub fn into_provenance(
        &self,
        registry: &plateforce_core::provenance::RegistryStamp,
        acquisition_complete: bool,
        depends_on: Vec<plateforce_core::Provenance>,
    ) -> plateforce_core::Provenance {
        use plateforce_core::provenance::{ChoiceRecord, ParameterRecord, RegistryStamp};

        // Destructured without a rest pattern, so a fact added to the stamp is a compile error
        // here rather than one this record quietly stops carrying.
        let RegistryStamp {
            version: registry_version,
            declared_version: registry_declared_version,
            digest: registry_digest,
        } = registry.clone();

        // The rule recorded a source per name as it read it. Anything absent was never read
        // by this rule, so it takes the weakest claim rather than being asserted as stated.
        let source_of = |name: &str| {
            self.parameter_sources
                .get(name)
                .copied()
                .unwrap_or(ParameterSource::Assumed)
        };

        plateforce_core::Provenance {
            method_id: self.method_id.clone(),
            // Settled where the rule was bound, from what the caller claimed about it, rather
            // than inferred here from the fields that happen to be beside it. A rule a
            // pipeline named, a rule accepted from the recommendation and a rule that ran
            // because nobody named one were none of them picked off a list by the reader, and
            // recording any of them as stated puts the reader's signature on somebody else's
            // choice.
            method_source: self.method_source,
            parameters: self
                .quantities()
                .into_iter()
                .map(|(name, value)| ParameterRecord {
                    source: source_of(&name),
                    name,
                    value,
                })
                .collect(),
            choices: self
                .enumerated_choices()
                .into_iter()
                .map(|(name, value)| ChoiceRecord {
                    source: source_of(&name),
                    name,
                    value,
                })
                .collect(),
            depends_on,
            registry_version,
            registry_declared_version,
            registry_digest,
            acquisition_complete,
            not_read: self.unread_parameters.clone(),
            manual_override: self.manual_override,
            registry_entry: self.registry_backed,
            composed_from: None,
            preset: self.preset.clone(),
        }
    }

    /// Names whose value the rule fell back to rather than being given. Derived from the
    /// recorded sources, so the list and the record cannot disagree.
    pub fn assumed_parameters(&self) -> Vec<String> {
        self.parameter_sources
            .iter()
            .filter(|(_, source)| **source == ParameterSource::Assumed)
            .map(|(name, _)| name.clone())
            .collect()
    }

    pub fn enumerated_choices(&self) -> Vec<(String, String)> {
        self.bound_parameters
            .iter()
            .filter(|(name, _)| !self.numeric_values.contains_key(name))
            .cloned()
            .collect()
    }
}

pub(crate) fn bound_method(
    method_id: &str,
    values: BoundValues,
    registry_backed: bool,
    manual_override: bool,
) -> BoundMethod {
    BoundMethod {
        method_id: method_id.to_string(),
        bound_parameters: values.parameters,
        parameter_sources: values.sources,
        unread_parameters: values.unread,
        registry_backed,
        manual_override,
        preset: values.preset,
        method_source: values.method_source,
        numeric_values: values.numbers,
    }
}

/// One rule's reading, split into the entry it is recorded under and the operator entries
/// it composed, each carrying only the values that belong to it.
///
/// Written once and called by both landmark slots. Each supplies its own name-to-entry map,
/// because the onset and takeoff operator families are separate registry entries and a
/// takeoff parameter recorded against an onset operator is a value filed under a construct
/// it never touched. The split itself is one rule, so it has one home.
pub(crate) fn bound_with_operators(
    method_id: &str,
    values: BoundValues,
    operator_for: fn(&str) -> Option<&'static str>,
    backed: impl Fn(&str) -> bool,
    manual_override: bool,
) -> Vec<BoundMethod> {
    let mut composed: BTreeMap<&'static str, BoundValues> = BTreeMap::new();
    let adopted = values.preset;
    // The caller named the threshold rule, and the operators arrived with it, so the claim
    // about how the rule was chosen belongs to the row the caller actually named.
    let mut carried = BoundValues {
        unread: values.unread,
        method_source: values.method_source,
        ..Default::default()
    };

    for (name, shown) in values.parameters {
        let target = match operator_for(&name) {
            Some(operator) => composed.entry(operator).or_default(),
            None => &mut carried,
        };
        if let Some(source) = values.sources.get(&name) {
            target.sources.insert(name.clone(), *source);
        }
        if let Some(number) = values.numbers.get(&name) {
            target.numbers.insert(name.clone(), *number);
        }
        target.parameters.push((name, shown));
    }

    // Recorded under the entry a reader can look up, which for a compound name is the rule
    // it composes. The operator that name bound is in `composed` beside it, so nothing the
    // compound name carried is lost by not naming it.
    carried.preset = attribution_for(&carried, adopted.as_ref(), true);
    let recorded = crate::binding::records_under(method_id);
    let mut bound = vec![bound_method(
        recorded,
        carried,
        backed(recorded),
        manual_override,
    )];
    bound.extend(composed.into_iter().map(|(operator, mut read)| {
        read.preset = attribution_for(&read, adopted.as_ref(), false);
        // Nobody named an operator. The caller named the rule that composes it and the
        // operator arrived with it, so this row records the rule's own provenance rather
        // than the claim the caller made about the row they did name: there is one operator
        // per name in this build, so reaching it was entailed rather than chosen. Where the
        // pipeline the caller adopted published a value this operator ran, that pipeline is
        // what put the value on the path and the row says so.
        read.method_source = match read.preset {
            Some(_) => ParameterSource::Cited,
            None => ParameterSource::Assumed,
        };
        bound_method(operator, read, backed(operator), false)
    }));
    bound
}
