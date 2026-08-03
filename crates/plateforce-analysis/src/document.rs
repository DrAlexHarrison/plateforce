//! One analysed trial, in the shape every surface writes it.
//!
//! A terminal, an R session and a notebook reporting one result under different field names
//! is the same defect as two implementations of one method, one layer out from the maths.

use std::collections::BTreeMap;

use plateforce_core::Refusal;
use serde::Serialize;

use crate::quality::QualitySignal;
use crate::resolution::{BoundMethod, DeclinedRule, RuleRefusal};
use crate::response::{AnalysisResponse, Levels, Metric};
use crate::spread::SpreadResponse;

/// Where the trace came from, and what the reader had to be told about reading it.
#[derive(Debug, Clone, Serialize)]
pub struct TrialSource {
    pub name: String,
    pub rows_read: usize,
    /// Rows that carried a missing-data sentinel rather than a reading.
    pub sentinel_rows: usize,
}

/// One analysed trial and everything a surface reports about it.
///
/// Field order is the wire order, so two surfaces can be compared byte for byte rather than
/// approximately.
#[derive(Debug, Clone, Serialize)]
pub struct ResultDocument {
    pub plateforce_version: String,
    pub trial: TrialSource,
    pub registry_version: Option<String>,
    pub registry_digest: Option<String>,
    pub acquisition_complete: bool,
    pub weighing_start_index: usize,
    pub weighing_end_index: usize,
    pub onset_index: Option<usize>,
    pub takeoff_index: Option<usize>,
    pub touchdown_index: Option<usize>,
    pub metrics: Vec<Metric>,
    pub bound_methods: Vec<BoundMethod>,
    pub levels: Levels,
    pub signals: Vec<QualitySignal>,
    pub warnings: Vec<String>,
    /// Every rule that declined, carrying the fields a caller branches on rather than a
    /// sentence one surface formats and another cannot represent at all.
    pub refusals: Vec<Refusal>,
    /// The account each quantity gives of itself, keyed by the quantity. Supplied by the
    /// caller, which is the layer that holds the chain each account is written from.
    pub descriptions: BTreeMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spread: Option<SpreadResponse>,
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
    let refused = match &declined.refusal {
        RuleRefusal::Trial(error) => Refusal::from(error.clone()),
        RuleRefusal::Refused(refusal) => refusal.as_ref().clone(),
    };
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
    #[allow(clippy::too_many_arguments)]
    pub fn of(
        plateforce_version: impl Into<String>,
        trial: TrialSource,
        registry_version: Option<String>,
        registry_digest: Option<String>,
        acquisition_complete: bool,
        response: &AnalysisResponse,
        descriptions: BTreeMap<String, String>,
        spread: Option<SpreadResponse>,
    ) -> Self {
        let refusals = response.refusals.iter().map(refusal_from_rule).collect();

        Self {
            plateforce_version: plateforce_version.into(),
            trial,
            registry_version,
            registry_digest,
            acquisition_complete,
            weighing_start_index: response.weighing_start_index,
            weighing_end_index: response.weighing_end_index,
            onset_index: response.onset_index,
            takeoff_index: response.takeoff_index,
            touchdown_index: response.touchdown_index,
            metrics: response.metrics.clone(),
            bound_methods: response.bound_methods.clone(),
            levels: response.levels.clone(),
            signals: response.signals.clone(),
            warnings: response.warnings.clone(),
            refusals,
            descriptions,
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

    /// A rule that declined on a name rather than a number publishes the name it declined
    /// on. Every one of these used to publish `unknown_parameter` with the whole sentence
    /// in the `parameter` column, which named a fault the request had not committed.
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
