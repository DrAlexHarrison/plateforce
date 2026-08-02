//! The chain of methods behind one number, and where each value in it came from.

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
