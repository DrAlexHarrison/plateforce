//! One analysed trial, in the shape every surface writes it.
//!
//! A terminal, an R session and a notebook reporting one result under different field names
//! is the same defect as two implementations of one method, one layer out from the maths.

use std::collections::BTreeMap;

use plateforce_core::Refusal;
use serde::Serialize;

use crate::quality::QualitySignal;
use crate::resolution::{BoundMethod, DeclinedRule};
use crate::response::{AnalysisResponse, BoundGlobal, Levels, Metric};
use crate::spread::SpreadResponse;

/// Where the trace came from, and what the reader had to be told about reading it.
#[derive(Debug, Clone, Serialize)]
pub struct TrialSource {
    pub name: String,
    pub rows_read: usize,
    /// Rows reading the value the declared convention writes for a measurement that was not
    /// taken. The reader's own fact, because only the reader was told which convention to
    /// apply.
    ///
    /// On a jump trace the zero convention a vendor writes for a missing measurement is also
    /// the correct reading of an unloaded plate, so it matches the whole flight phase: one
    /// total over this and the rows carrying no number reads 160 on a recording whose real
    /// answer is 157 samples of flight and 3 samples of a gap.
    ///
    /// The gap is not here. It belongs to the recording rather than to the reader, so it is
    /// counted once by the engine and published as `samples_carrying_no_number` on every
    /// surface's result rather than by each reader for itself.
    ///
    /// Counted by `plateforce_core::signal::reported_samples`, which is where the policy
    /// lives for every surface.
    pub samples_matching_the_convention: usize,
}

/// One analysed trial and everything a surface reports about it.
///
/// Field order is the wire order, so two surfaces can be compared byte for byte rather than
/// approximately.
#[derive(Debug, Clone, Serialize)]
pub struct ResultDocument {
    pub plateforce_version: String,
    pub trial: TrialSource,
    /// The revision the caller pinned, and null when they pinned none. Null rather than
    /// absent: a missing key cannot be told apart from a surface that never carried the
    /// field, and a reader has no way to ask the document which happened.
    pub registry_version: Option<String>,
    /// The revision the registry names about itself, and null where it names none. What the
    /// data claims, never what the caller cited.
    pub registry_declared_version: Option<String>,
    pub registry_digest: Option<String>,
    /// What the plate and its settings were, as the caller stated them. Carried whole rather
    /// than as the completeness flag alone: a reader holding the flag knows the result cannot
    /// be declared to match another lab's and has no way to see which of the members it holds,
    /// and `Acquisition::missing` names the rest.
    pub acquisition: plateforce_core::Acquisition,
    pub acquisition_complete: bool,
    /// The saved plate the block above was filled from, absent when the caller typed the
    /// members or stated none. Absent rather than null for the reason `spread` is: a run with
    /// no saved plate behind it has nothing to attribute, where an unpinned registry revision
    /// is an answer every result owes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plate_profile: Option<plateforce_core::PlateProfileAttribution>,
    /// Samples of the recording that carried no number, from the engine rather than from the
    /// surface, so a terminal, a browser tab, a notebook and an R session answer it alike.
    pub samples_carrying_no_number: usize,
    pub weighing_start_index: usize,
    pub weighing_end_index: usize,
    pub onset_index: Option<usize>,
    pub takeoff_index: Option<usize>,
    pub touchdown_index: Option<usize>,
    pub metrics: Vec<Metric>,
    /// The intervals this run's boundary rules settled, each under the name a caller states to
    /// take a window over it. Carried on the document rather than only on the response so a
    /// notebook, an R session and a terminal can offer the same intervals a tab can.
    pub regions: Vec<crate::response::PlacedRegion>,
    pub bound_methods: Vec<BoundMethod>,
    /// What the request bound for the whole analysis. Beside `bound_methods` because a
    /// reader asking what produced a number asks both questions in one place, and no rule's
    /// row can answer this one.
    pub bound_globals: Vec<BoundGlobal>,
    pub levels: Levels,
    pub signals: Vec<QualitySignal>,
    pub warnings: Vec<String>,
    /// Every rule that declined, carrying the fields a caller branches on rather than a
    /// sentence one surface formats and another cannot represent at all.
    pub refusals: Vec<Refusal>,
    /// The account each quantity gives of itself, keyed by the quantity. Written by
    /// `plateforce_analysis::descriptions_of` from the response, never handed in, because a
    /// surface that supplied this block could supply an empty one and two of them did.
    ///
    /// Always written, empty included. An empty block is a run whose quantities all carried
    /// no value, which is a fact about the trial; a key a document sometimes omits cannot be
    /// told apart from a surface that never carried the field, which is the reason
    /// `registry_version` is written as null rather than left out.
    pub descriptions: BTreeMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spread: Option<SpreadResponse>,
}

/// One swept quantity and everything a surface reports about it, in the shape every surface
/// writes it.
///
/// A spread that leaves inside a `ResultDocument` inherits that document's identity. A spread
/// that leaves on its own carries these three fields, so it still says which build and which
/// registry produced it.
///
/// The three identity fields are spelled as `ResultDocument` spells them and are supplied by
/// the calling surface for the same reason: the layer that loaded the registry is the layer
/// that knows which one it loaded. A surface that derived them here would be answering for a
/// registry it never read.
///
/// Flattened, so the fifteen keys a reader already reads stay where they are and the identity
/// arrives beside them rather than nesting the whole document one level deeper.
#[derive(Debug, Clone, Serialize)]
pub struct SpreadDocument {
    pub plateforce_version: String,
    /// The revision the caller pinned, and null when they pinned none.
    pub registry_version: Option<String>,
    /// The revision the registry names about itself, and null where it names none. What the
    /// data claims, never what the caller cited.
    pub registry_declared_version: Option<String>,
    pub registry_digest: Option<String>,
    #[serde(flatten)]
    pub spread: SpreadResponse,
}

impl SpreadDocument {
    /// The document for one sweep, taking its identity from the surface that loaded the
    /// registry and everything else from the sweep itself.
    ///
    /// The stamp arrives whole rather than as loose options, for the reason `ResultDocument`
    /// takes it whole: three same-typed `Option<String>` passed positionally is a signature
    /// that accepts a transposed pair and compiles, and this exact pair has been transposed
    /// before, on two surfaces, publishing the registry's own claim under the caller's name.
    pub fn of(
        plateforce_version: impl Into<String>,
        registry: &plateforce_core::provenance::RegistryStamp,
        spread: SpreadResponse,
    ) -> Self {
        // Destructured without a rest pattern, so a fact added to the stamp is a compile error
        // here rather than one this document quietly stops carrying.
        let plateforce_core::provenance::RegistryStamp {
            version: registry_version,
            declared_version: registry_declared_version,
            digest: registry_digest,
        } = registry.clone();

        Self {
            plateforce_version: plateforce_version.into(),
            registry_version,
            registry_declared_version,
            registry_digest,
            spread,
        }
    }
}

/// What a rule's refusal is, as the typed record rather than as prose.
///
/// Neither arm decides a code here. `TrialError` already carries its own and generates its
/// own sentence, and a rule that built a `Refusal` chose the code it was declining under.
/// The one thing this adds is the identity the rule could not know: which id a caller
/// reached it by, and which construct that id was bound to.
///
/// The slot is named as the registry names constructs. The binding table's own words,
/// `weighing` and `onset`, resolve to nothing in the registry, so a caller handed one has a
/// name it cannot look up.
pub fn refusal_from_rule(declined: &DeclinedRule) -> Refusal {
    let refused = Refusal::from(declined.refusal.clone());
    let named = if refused.method_id.is_empty() {
        refused.under(&declined.method_id)
    } else {
        refused
    };
    named.in_slot(declined.construct)
}

impl ResultDocument {
    /// The document for one analysed trial. Everything derivable from the response is taken
    /// from it, and the rest is what only the calling surface knows.
    ///
    /// The accounts are derived here rather than accepted here. Both callers passed an empty
    /// map, so a terminal and a browser tab published a result with no account of any number
    /// in it, and neither was doing anything a signature could catch.
    #[allow(clippy::too_many_arguments)]
    pub fn of(
        plateforce_version: impl Into<String>,
        trial: TrialSource,
        registry: &plateforce_core::provenance::RegistryStamp,
        capture: &plateforce_core::Capture,
        response: &AnalysisResponse,
        spread: Option<SpreadResponse>,
    ) -> Self {
        let refusals = response.refusals.iter().map(refusal_from_rule).collect();

        // Destructured without a rest pattern, so a fact added to the stamp is a compile error
        // here rather than one this document quietly stops carrying.
        let plateforce_core::provenance::RegistryStamp {
            version: registry_version,
            declared_version: registry_declared_version,
            digest: registry_digest,
        } = registry.clone();

        // Destructured for the same reason, and the completeness flag is read off the block
        // rather than taken from the caller: a surface that answered it for itself is how two
        // of the five came to publish a literal `false` beside a block nobody could give them.
        let plateforce_core::Capture {
            acquisition,
            plate_profile,
        } = capture.clone();

        Self {
            plateforce_version: plateforce_version.into(),
            trial,
            registry_version,
            registry_declared_version,
            registry_digest,
            descriptions: crate::accounts_of(response, registry, acquisition.is_complete()),
            acquisition_complete: acquisition.is_complete(),
            acquisition,
            plate_profile,
            samples_carrying_no_number: response.samples_carrying_no_number,
            weighing_start_index: response.weighing_start_index,
            weighing_end_index: response.weighing_end_index,
            onset_index: response.onset_index,
            takeoff_index: response.takeoff_index,
            touchdown_index: response.touchdown_index,
            metrics: response.metrics.clone(),
            regions: response.regions.clone(),
            bound_methods: response.bound_methods.clone(),
            bound_globals: response.bound_globals.clone(),
            levels: response.levels.clone(),
            signals: response.signals.clone(),
            warnings: response.warnings.clone(),
            refusals,
            spread,
        }
    }

    /// True when no rule declined, so a caller can branch without reading a sentence.
    pub fn every_rule_ran(&self) -> bool {
        self.refusals.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::binding::{ONSET_CONSTRUCT, TAKEOFF_CONSTRUCT, WEIGHING_CONSTRUCT};
    use crate::resolution::RuleRefusal;
    use plateforce_core::{RefusalCode, TrialError};

    fn declined(construct: &'static str, method_id: &str, refusal: RuleRefusal) -> DeclinedRule {
        DeclinedRule {
            construct,
            method_id: method_id.to_string(),
            refusal,
        }
    }

    #[test]
    fn a_declined_rule_arrives_as_fields_rather_than_a_sentence() {
        let refused = refusal_from_rule(&declined(
            ONSET_CONSTRUCT,
            "onset.threshold.noise_relative",
            RuleRefusal::Trial(TrialError::NoCrossing {
                method_id: "onset.threshold.noise_relative".to_string(),
                parameter: "k".to_string(),
                value: 5.0,
                search_bound_seconds: 2.5,
            }),
        ));

        assert_eq!(refused.code, RefusalCode::NoCrossing);
        assert_eq!(refused.parameter.as_deref(), Some("k"));
        assert_eq!(refused.value, Some(5.0));
        // The registry declares `movement_onset` and declares no `onset`, so this is the
        // spelling a caller can look up.
        assert_eq!(refused.slot.as_deref(), Some("movement_onset"));
        // The number a caller branches on is a number, not a substring of the sentence.
        assert_eq!(refused.detail["search_bound_seconds"], 2.5);
    }

    #[test]
    fn a_refusal_that_names_no_rule_is_stamped_with_the_one_the_slot_ran() {
        let refused = refusal_from_rule(&declined(
            TAKEOFF_CONSTRUCT,
            "takeoff.threshold.absolute_force",
            RuleRefusal::Trial(TrialError::Empty),
        ));
        assert_eq!(refused.method_id, "takeoff.threshold.absolute_force");
        assert_eq!(refused.slot.as_deref(), Some("takeoff"));
    }

    /// A rule that declined on a name rather than a number publishes the name it declined on.
    #[test]
    fn a_value_the_rule_will_not_take_keeps_its_own_code_across_the_boundary() {
        let refused = refusal_from_rule(&declined(
            WEIGHING_CONSTRUCT,
            "bwepoch.fixed_window",
            RuleRefusal::Refused(Box::new(Refusal::name_not_accepted(
                "",
                "dispersion",
                "unbiased",
                vec!["population".to_string(), "sample".to_string()],
            ))),
        ));
        assert_eq!(refused.code, RefusalCode::ValueNotAccepted);
        assert_eq!(refused.named_value.as_deref(), Some("unbiased"));
        assert_eq!(refused.parameter.as_deref(), Some("dispersion"));
        // The rule that declined did not know which entry reached it, so the boundary
        // stamped the id on and the sentence was regenerated under it.
        assert_eq!(refused.method_id, "bwepoch.fixed_window");
        assert_eq!(refused.slot.as_deref(), Some("system_weight"));
    }
}
