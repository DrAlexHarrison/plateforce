//! What one rule read while it ran, and where each value came from.
//!
//! Built from the request instead, a fingerprint omits every value the rule chose for
//! itself, which is the silent default this project exists to document.

use std::collections::{BTreeMap, BTreeSet};

use plateforce_core::takeoff::ResidualComparison;
use plateforce_core::provenance::ParameterSource;
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

#[derive(Debug, Clone, Default)]
pub(crate) struct BoundValues {
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
                let offered: Vec<&str> = accepted.iter().map(|(label, _)| *label).collect();
                RuleRefusal::Stated(format!(
                    "{name} takes one of {offered:?}, and '{chosen}' is not one of them"
                ))
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

/// Why a landmark rule produced nothing.
///
/// The sentence goes to `warnings` for somebody reading the trace. This carries the core's
/// own error beside it wherever there is one, so a caller can branch on the method and the
/// parameter that failed rather than parse the sentence back apart.
#[derive(Debug, Clone)]
pub enum RuleRefusal {
    Trial(plateforce_core::TrialError),
    Stated(String),
}

impl std::fmt::Display for RuleRefusal {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RuleRefusal::Trial(error) => write!(formatter, "{error}"),
            RuleRefusal::Stated(message) => formatter.write_str(message),
        }
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
