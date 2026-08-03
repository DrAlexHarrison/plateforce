//! What one rule read while it ran, and where each value came from.
//!
//! Built from the request instead, a fingerprint omits every value the rule chose for
//! itself, which is the silent default this project exists to document.

use std::collections::{BTreeMap, BTreeSet};

use plateforce_core::provenance::ParameterSource;
use plateforce_core::takeoff::ResidualComparison;
use plateforce_core::DispersionEstimator;
use serde::Serialize;

pub(crate) struct Resolution<'a> {
    parameters: &'a BTreeMap<String, f64>,
    options: &'a BTreeMap<String, String>,
    /// Names the caller says it accepted from the registry's recommendation, and names it
    /// filled from a default with nobody asked. A rule cannot tell either from the number.
    recommended: &'a BTreeSet<String>,
    from_registry_default: &'a BTreeSet<String>,
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
#[derive(Debug, Clone, Default)]
pub struct BoundValues {
    pub parameters: Vec<(String, String)>,
    pub sources: BTreeMap<String, ParameterSource>,
    pub unread: Vec<String>,
    pub numbers: BTreeMap<String, f64>,
}

impl<'a> Resolution<'a> {
    pub(crate) fn over(
        parameters: &'a BTreeMap<String, f64>,
        options: &'a BTreeMap<String, String>,
        recommended: &'a BTreeSet<String>,
        from_registry_default: &'a BTreeSet<String>,
    ) -> Self {
        Self {
            parameters,
            options,
            recommended,
            from_registry_default,
            read: Vec::new(),
            sources: BTreeMap::new(),
            consulted: BTreeSet::new(),
            numbers: BTreeMap::new(),
        }
    }

    /// What the caller claimed about a name it did supply.
    ///
    /// A value present in the request is `stated` unless the caller said otherwise, because
    /// only the caller knows whether a number was typed, accepted in bulk, or filled by an
    /// interface on the reader's behalf.
    fn stated_source(&self, name: &str) -> ParameterSource {
        if self.recommended.contains(name) {
            ParameterSource::Recommended
        } else if self.from_registry_default.contains(name) {
            ParameterSource::Assumed
        } else {
            ParameterSource::Stated
        }
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
    /// beside a number a different rule produced, which is the defect this project
    /// documents wearing our own badge.
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
        }
    }
}

pub(crate) fn format_number(value: f64) -> String {
    if value.fract().abs() < 1e-9 {
        format!("{value:.0}")
    } else {
        format!("{value}")
    }
}

pub(crate) fn dispersion_label(dispersion: DispersionEstimator) -> &'static str {
    match dispersion {
        DispersionEstimator::Population => "population",
        DispersionEstimator::Sample => "sample",
    }
}

/// Why a rule produced nothing.
///
/// The sentence goes to `warnings` for somebody reading the trace. Both variants carry the
/// fields a caller branches on, so no rule reaches a surface with a sentence and nothing
/// else. A `Stated(String)` variant used to sit beside these two, and every refusal that
/// took it was published under one code chosen at the boundary, which was the wrong code
/// for every producer of it.
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

/// One rule that declined, with everything the rule itself could not know: which construct
/// it was filling and which id a caller reached it by.
///
/// Both were previously recovered at each surface by matching the start of a method id
/// against a table of prefixes, whose last arm answered `takeoff.` for every name it did
/// not recognise. Two surfaces carried a copy of that table.
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
/// Written here rather than left to each reader, because this used to be skipped entirely:
/// a rule that declined reached R and the browser as a sentence in `warnings` and nothing
/// else, so thirteen condition classes the R package publishes could not be raised on any
/// landmark rule. A refusal that cannot cross is a result without its method.
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
        registry_version: Option<String>,
        registry_digest: Option<String>,
        acquisition_complete: bool,
        depends_on: Vec<plateforce_core::Provenance>,
    ) -> plateforce_core::Provenance {
        use plateforce_core::provenance::{ChoiceRecord, ParameterRecord};

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
            method_source: ParameterSource::Stated,
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
            registry_digest,
            acquisition_complete,
            not_read: self.unread_parameters.clone(),
            manual_override: self.manual_override,
            registry_entry: self.registry_backed,
            composed_from: None,
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
        numeric_values: values.numbers,
    }
}
