//! The chain of methods behind one number, and where each value in it came from.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::Provenance;

/// Declares the enum and the list of every variant from one source, so the vocabulary a
/// surface reports cannot fall behind the sources the build can record.
macro_rules! parameter_sources {
    (
        $(#[$enum_note:meta])*
        pub enum $name:ident { $( $(#[$note:meta])* $variant:ident ),+ $(,)? }
    ) => {
        $(#[$enum_note])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
        #[serde(rename_all = "snake_case")]
        pub enum $name {
            $( $(#[$note])* $variant, )+
        }

        impl $name {
            /// Every source this build can record.
            pub const ALL: &'static [$name] = &[ $( $name::$variant, )+ ];
        }
    };
}

parameter_sources! {
    /// Where a bound value came from.
    ///
    /// A value the caller typed and a value the interface pre-filled from the registry move
    /// the number by exactly the same amount, and recording the second as the first is a
    /// default wearing the user's signature. They are kept apart because each answers a
    /// different question a reader of a methods section asks.
    pub enum ParameterSource {
        /// The caller supplied the value and claimed no other source for it.
        Stated,
        /// A registry default was used with nobody asked, by the rule or by the interface.
        Assumed,
        /// The rule computed it from this trace.
        Measured,
        /// The user accepted the registry's recommendation as an explicit act.
        Recommended,
        /// No act has happened. The value exists to be looked at, and a result resting on one
        /// cannot leave the building.
        Provisional,
        /// A named published pipeline the caller adopted supplied the value. The caller chose
        /// the pipeline by its id and its citation, not this value.
        Cited,
    }
}

impl ParameterSource {
    /// Whether a value from this source leaves a result unfit to export, fingerprint or
    /// cite. Only an unmade decision does. An accepted recommendation is a choice, and so
    /// is adopting a published pipeline.
    pub fn taints_the_record(self) -> bool {
        matches!(self, ParameterSource::Provisional)
    }
}

/// The named published pipeline a step's rule and values were adopted from.
///
/// Carried per step rather than per result, because a pipeline binds the slots its source
/// states and nothing else. A step its source is silent about carries none of this, and
/// attributing that step to the pipeline would be manufacturing provenance.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PresetAttribution {
    /// The id the caller named, as the registry files it.
    pub id: String,
    /// Values this pipeline states for this rule that the caller replaced. `parameters` and
    /// `choices` carry what ran, and these carry what the source published, so a reader sees
    /// both numbers rather than having to look the pipeline up to learn what was displaced.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub superseded_parameters: BTreeMap<String, f64>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub superseded_options: BTreeMap<String, String>,
}

impl PresetAttribution {
    pub fn of(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            superseded_parameters: BTreeMap::new(),
            superseded_options: BTreeMap::new(),
        }
    }

    /// Whether the caller replaced anything this pipeline states for this rule.
    pub fn was_overridden(&self) -> bool {
        !self.superseded_parameters.is_empty() || !self.superseded_options.is_empty()
    }

    /// Every name the caller replaced, sorted, so a surface listing them reads the same on
    /// every run.
    pub fn superseded_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self
            .superseded_parameters
            .keys()
            .chain(self.superseded_options.keys())
            .cloned()
            .collect();
        names.sort();
        names
    }
}

/// A numeric value a rule was bound to, and where it came from.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParameterRecord {
    pub name: String,
    pub value: f64,
    pub source: ParameterSource,
}

/// A choice between named alternatives, and where it came from. Population against sample
/// standard deviation moves the number as far as a numeric parameter does.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChoiceRecord {
    pub name: String,
    pub value: String,
    pub source: ParameterSource,
}

/// Which part of a step is waiting on a decision, so a refusal can name it the way the
/// reader met it rather than calling every open question a parameter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProvisionalKind {
    /// No rule was picked for the slot.
    Method,
    Parameter,
    Choice,
}

/// One open decision, named by the step it sits on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvisionalSource {
    pub method_id: String,
    pub name: String,
    pub kind: ProvisionalKind,
}

/// Every decision still open anywhere upstream of this result, deduplicated and ordered so
/// a refusal reads the same on every run.
///
/// The parameter holding a result back usually sits on an upstream step, so a check that
/// read only the top of the chain would clear a result whose onset rule nobody chose.
pub fn provisional_sources(provenance: &Provenance) -> Vec<ProvisionalSource> {
    let mut found: Vec<ProvisionalSource> = Vec::new();
    for step in provenance.flattened() {
        let method_id = step.method_id.clone();
        if step.method_source.taints_the_record() {
            found.push(ProvisionalSource {
                method_id: method_id.clone(),
                name: method_id.clone(),
                kind: ProvisionalKind::Method,
            });
        }
        for parameter in &step.parameters {
            if parameter.source.taints_the_record() {
                found.push(ProvisionalSource {
                    method_id: method_id.clone(),
                    name: parameter.name.clone(),
                    kind: ProvisionalKind::Parameter,
                });
            }
        }
        for choice in &step.choices {
            if choice.source.taints_the_record() {
                found.push(ProvisionalSource {
                    method_id: method_id.clone(),
                    name: choice.name.clone(),
                    kind: ProvisionalKind::Choice,
                });
            }
        }
    }
    found
        .sort_by(|left, right| (&left.method_id, &left.name).cmp(&(&right.method_id, &right.name)));
    found.dedup_by(|left, right| left.method_id == right.method_id && left.name == right.name);
    found
}

/// Whether this result rests on a decision nobody has made, and therefore cannot be
/// exported, fingerprinted or cited.
pub fn is_provisional(provenance: &Provenance) -> bool {
    !provisional_sources(provenance).is_empty()
}

/// A provenance and the provenances of the results it was computed from.
///
/// Jump height moves with the onset rule and the weighing epoch as well as with the
/// jump-height formula, so a result that named only the last step would understate what
/// produced it.
#[derive(Debug, Clone, PartialEq)]
pub struct ProvenanceChain {
    pub provenance: Provenance,
    /// Choices that select between named alternatives rather than between numbers, such
    /// as population against sample standard deviation. `Provenance::bound_parameters` is
    /// a list of `(String, f64)` and cannot hold one.
    pub enumerated_choices: Vec<(String, String)>,
    pub depends_on: Vec<ProvenanceChain>,
}

impl ProvenanceChain {
    pub fn leaf(provenance: Provenance) -> Self {
        Self {
            provenance,
            enumerated_choices: Vec::new(),
            depends_on: Vec::new(),
        }
    }

    pub fn with_inputs(provenance: Provenance, depends_on: Vec<ProvenanceChain>) -> Self {
        Self {
            provenance,
            enumerated_choices: Vec::new(),
            depends_on,
        }
    }

    pub fn choosing(mut self, choices: Vec<(String, String)>) -> Self {
        self.enumerated_choices = choices;
        self
    }

    /// This step and every one upstream of it, depth first.
    ///
    /// The parameter that moved a downstream number usually sits on an upstream step: the
    /// k that placed onset is on the onset entry, not on the time to takeoff derived from it.
    pub fn flattened(&self) -> Vec<&ProvenanceChain> {
        let mut collected = Vec::new();
        self.collect_into(&mut collected);
        collected
    }

    fn collect_into<'a>(&'a self, into: &mut Vec<&'a ProvenanceChain>) {
        into.push(self);
        for input in &self.depends_on {
            input.collect_into(into);
        }
    }

    /// The step naming this method anywhere in this chain, or None when the chain does not
    /// include it.
    pub fn step_of(&self, method_id: &str) -> Option<&ProvenanceChain> {
        self.flattened()
            .into_iter()
            .find(|step| step.provenance.method_id == method_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn step(method_id: &str) -> Provenance {
        Provenance {
            acquisition_complete: true,
            ..Provenance::of(method_id)
        }
    }

    /// A three-step chain, jump height over onset over the weighing epoch, with every value
    /// on the deepest step carrying `source`.
    fn chain_with_leaf_values(source: ParameterSource) -> Provenance {
        let mut leaf = step("bwepoch.fixed_window");
        leaf.parameters.push(ParameterRecord {
            name: "duration_seconds".to_string(),
            value: 1.0,
            source,
        });
        leaf.choices.push(ChoiceRecord {
            name: "sd_convention".to_string(),
            value: "population".to_string(),
            source,
        });
        let mut middle = step("onset.threshold.noise_relative");
        middle.depends_on.push(leaf);
        let mut root = step("jumpheight.takeoff.impulse_momentum");
        root.depends_on.push(middle);
        root
    }

    #[test]
    fn a_decision_three_deep_still_holds_the_result_back() {
        let open = provisional_sources(&chain_with_leaf_values(ParameterSource::Provisional));
        let named: Vec<&str> = open.iter().map(|source| source.name.as_str()).collect();
        assert_eq!(named, ["duration_seconds", "sd_convention"]);
        assert!(open
            .iter()
            .all(|source| source.method_id == "bwepoch.fixed_window"));
        assert!(is_provisional(&chain_with_leaf_values(
            ParameterSource::Provisional
        )));
    }

    #[test]
    fn a_choice_the_reader_made_does_not_hold_the_result_back() {
        for made in [
            ParameterSource::Stated,
            ParameterSource::Assumed,
            ParameterSource::Measured,
            ParameterSource::Recommended,
            ParameterSource::Cited,
        ] {
            let chain = chain_with_leaf_values(made);
            assert!(
                provisional_sources(&chain).is_empty(),
                "{made:?} held the result back"
            );
            assert!(!is_provisional(&chain), "{made:?}");
        }
    }

    #[test]
    fn a_slot_running_under_no_chosen_rule_is_named_by_its_method() {
        let mut root = step("jumpheight.takeoff.impulse_momentum");
        let mut onset = step("onset.threshold.noise_relative");
        onset.method_source = ParameterSource::Provisional;
        root.depends_on.push(onset);

        let open = provisional_sources(&root);
        assert_eq!(open.len(), 1);
        assert_eq!(open[0].kind, ProvisionalKind::Method);
        assert_eq!(open[0].method_id, "onset.threshold.noise_relative");
    }

    #[test]
    fn one_open_decision_reached_twice_is_reported_once() {
        let shared = chain_with_leaf_values(ParameterSource::Provisional)
            .depends_on
            .remove(0);
        let mut root = step("jumpheight.takeoff.impulse_momentum");
        root.depends_on.push(shared.clone());
        root.depends_on.push(shared);

        assert_eq!(provisional_sources(&root).len(), 2);
    }

    /// The record as the shared cross-surface schema spells it, pasted rather than built, so
    /// this fails if the shape drifts from the document every surface is written against.
    const SHARED_SCHEMA_RECORD: &str = r#"{
      "method_id": "jumpheight.takeoff.impulse_momentum",
      "parameters": [
        { "name": "gravity_meters_per_second_squared", "value": 9.80665, "source": "assumed" }
      ],
      "choices": [],
      "registry_version": "2026-07-25",
      "registry_digest": "content-3c2a669c2ed48199",
      "acquisition_complete": false,
      "depends_on": [
        {
          "method_id": "onset.threshold.noise_relative",
          "parameters": [ { "name": "k", "value": 5.0, "source": "stated" } ],
          "choices": [
            { "name": "degenerate_band", "value": "refuse", "source": "assumed" },
            { "name": "sd_convention", "value": "sample", "source": "assumed" }
          ],
          "registry_version": "2026-07-25",
          "registry_digest": "content-3c2a669c2ed48199",
          "acquisition_complete": false,
          "depends_on": []
        }
      ]
    }"#;

    #[test]
    fn a_record_written_to_the_shared_schema_reads_back_unchanged() {
        let parsed: Provenance = serde_json::from_str(SHARED_SCHEMA_RECORD).unwrap();
        assert_eq!(parsed.method_id, "jumpheight.takeoff.impulse_momentum");
        assert_eq!(parsed.depends_on.len(), 1);
        assert_eq!(parsed.depends_on[0].choices.len(), 2);
        assert!(!parsed.acquisition_complete);

        // Re-serialising adds the fields beyond the shared schema, so the assertion is that
        // the record survives the trip, not that the two texts match.
        let written = serde_json::to_string(&parsed).unwrap();
        let read_back: Provenance = serde_json::from_str(&written).unwrap();
        assert_eq!(parsed, read_back);
    }

    #[test]
    fn the_shared_schema_keys_are_written_in_the_order_every_surface_compares() {
        let written = serde_json::to_string(
            &serde_json::from_str::<Provenance>(SHARED_SCHEMA_RECORD).unwrap(),
        )
        .unwrap();
        let shared = [
            "\"method_id\"",
            "\"parameters\"",
            "\"choices\"",
            "\"registry_version\"",
            "\"registry_digest\"",
            "\"acquisition_complete\"",
            "\"depends_on\"",
        ];
        let mut previous = 0;
        for key in shared {
            let at = written
                .find(key)
                .unwrap_or_else(|| panic!("{key} is not written"));
            assert!(at > previous, "{key} is out of the shared order");
            previous = at;
        }
    }

    #[test]
    fn every_source_value_carries_its_declared_wire_spelling() {
        let spellings = [
            (ParameterSource::Stated, "\"stated\""),
            (ParameterSource::Assumed, "\"assumed\""),
            (ParameterSource::Measured, "\"measured\""),
            (ParameterSource::Recommended, "\"recommended\""),
            (ParameterSource::Provisional, "\"provisional\""),
            (ParameterSource::Cited, "\"cited\""),
        ];
        for source in ParameterSource::ALL {
            assert!(
                spellings.iter().any(|(pinned, _)| pinned == source),
                "{source:?} has no wire spelling pinned, {} of {} do",
                spellings.len(),
                ParameterSource::ALL.len()
            );
        }
        for (source, expected) in spellings {
            assert_eq!(serde_json::to_string(&source).unwrap(), expected);
            assert_eq!(
                serde_json::from_str::<ParameterSource>(expected).unwrap(),
                source
            );
        }
    }

    #[test]
    fn only_an_unmade_decision_taints_the_record() {
        assert!(ParameterSource::Provisional.taints_the_record());
        for kept in ParameterSource::ALL
            .iter()
            .filter(|source| **source != ParameterSource::Provisional)
        {
            assert!(!kept.taints_the_record(), "{kept:?}");
        }
    }

    #[test]
    fn flattened_reaches_every_depth() {
        let chain = ProvenanceChain::with_inputs(
            step("jumpheight.takeoff.impulse_momentum"),
            vec![ProvenanceChain::with_inputs(
                step("onset.threshold.noise_relative"),
                vec![ProvenanceChain::leaf(step("bwepoch.fixed_window"))],
            )],
        );

        let ids: Vec<&str> = chain
            .flattened()
            .iter()
            .map(|step| step.provenance.method_id.as_str())
            .collect();
        assert_eq!(
            ids,
            [
                "jumpheight.takeoff.impulse_momentum",
                "onset.threshold.noise_relative",
                "bwepoch.fixed_window",
            ]
        );
    }

    #[test]
    fn a_method_three_deep_is_still_found() {
        let chain = ProvenanceChain::with_inputs(
            step("jumpheight.takeoff.impulse_momentum"),
            vec![ProvenanceChain::with_inputs(
                step("onset.threshold.noise_relative"),
                vec![ProvenanceChain::leaf(step("bwepoch.fixed_window"))],
            )],
        );

        assert!(chain.step_of("bwepoch.fixed_window").is_some());
        assert!(chain.step_of("takeoff.threshold.absolute_force").is_none());
    }
}
