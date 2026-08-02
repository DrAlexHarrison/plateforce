//! One analysed trial, in the shape every surface writes it.
//!
//! A terminal, an R session and a notebook reporting one result under different field names
//! is the same defect as two implementations of one method, one layer out from the maths.

use std::collections::BTreeMap;

use plateforce_core::Refusal;
use serde::Serialize;

use crate::quality::QualitySignal;
use crate::resolution::{BoundMethod, RuleRefusal};
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
/// `TrialError` already carries every field and generates its own sentence, so this reads
/// them off it rather than destructuring the error a second time. A refusal that names no
/// rule of its own is stamped with the rule the slot was bound to.
pub fn refusal_from_rule(slot: &str, refusal: &RuleRefusal, bound_method_id: &str) -> Refusal {
    match refusal {
        RuleRefusal::Trial(error) => {
            let refused = Refusal::from(error.clone());
            let named = if refused.method_id.is_empty() {
                refused.under(bound_method_id)
            } else {
                refused
            };
            named.in_slot(slot)
        }
        RuleRefusal::Stated(message) => {
            Refusal::unknown_parameter(bound_method_id, message.clone(), Vec::new()).in_slot(slot)
        }
    }
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
        let refusals = response
            .refusals
            .iter()
            .map(|(slot, refusal)| {
                let bound = response
                    .bound_methods
                    .iter()
                    .find(|bound| bound.method_id.starts_with(slot_prefix(slot)))
                    .map(|bound| bound.method_id.as_str())
                    .unwrap_or_default();
                refusal_from_rule(slot, refusal, bound)
            })
            .collect();

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

/// The registry's own word for the slot, which is what a bound method id begins with.
fn slot_prefix(slot: &str) -> &'static str {
    match slot {
        "weighing" | "system_weight" => "bwepoch.",
        "onset" | "movement_onset" => "onset.",
        _ => "takeoff.",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use plateforce_core::{RefusalCode, TrialError};

    #[test]
    fn a_declined_rule_arrives_as_fields_rather_than_a_sentence() {
        let refused = refusal_from_rule(
            "onset",
            &RuleRefusal::Trial(TrialError::NoCrossing {
                method_id: "onset.threshold.noise_relative".to_string(),
                parameter: "k".to_string(),
                value: 5.0,
                search_bound_seconds: 2.5,
            }),
            "onset.threshold.noise_relative",
        );

        assert_eq!(refused.code, RefusalCode::NoCrossing);
        assert_eq!(refused.parameter.as_deref(), Some("k"));
        assert_eq!(refused.value, Some(5.0));
        assert_eq!(refused.slot.as_deref(), Some("onset"));
        // The number a caller branches on is a number, not a substring of the sentence.
        assert_eq!(refused.detail["search_bound_seconds"], 2.5);
    }

    #[test]
    fn a_refusal_that_names_no_rule_is_stamped_with_the_one_the_slot_ran() {
        let refused = refusal_from_rule(
            "takeoff",
            &RuleRefusal::Trial(TrialError::Empty),
            "takeoff.threshold.absolute_force",
        );
        assert_eq!(refused.method_id, "takeoff.threshold.absolute_force");
        assert_eq!(refused.slot.as_deref(), Some("takeoff"));
    }

    #[test]
    fn a_request_for_something_not_on_offer_says_which_slot_asked() {
        let refused = refusal_from_rule(
            "weighing",
            &RuleRefusal::Stated("duration is not a parameter of this rule".to_string()),
            "bwepoch.fixed_window",
        );
        assert_eq!(refused.code, RefusalCode::UnknownParameter);
        assert_eq!(refused.slot.as_deref(), Some("weighing"));
    }
}
