//! Why the software declined to produce a number.
//!
//! Every field a caller can branch on is a field, never a substring of `message`, and the
//! sentence is generated here so a refusal reads the same in a browser tab, a traceback, an
//! R condition and a terminal.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RefusalCode {
    NoCrossing,
    CollapsedBand,
    MethodNotImplemented,
    UnknownParameter,
    ParameterNotFinite,
    TraceTooShort,
    ColumnNotFound,
    SentinelConventionUnknown,
    RegistryInvalid,
    /// A result was asked for while a choice the registry forces is still open.
    DecisionNotMade,
}

/// Exit status for a refusal, from `sysexits.h`, which is the convention every workflow
/// manager already reads.
///
/// The match takes no wildcard arm, so a new code has to be ruled on rather than falling
/// through to whichever status happened to be last.
pub fn exit_code(code: RefusalCode) -> i32 {
    match code {
        RefusalCode::NoCrossing
        | RefusalCode::CollapsedBand
        | RefusalCode::TraceTooShort
        | RefusalCode::ColumnNotFound => 65,
        RefusalCode::MethodNotImplemented
        | RefusalCode::UnknownParameter
        | RefusalCode::ParameterNotFinite
        | RefusalCode::SentinelConventionUnknown
        | RefusalCode::DecisionNotMade => 64,
        RefusalCode::RegistryInvalid => 78,
    }
}

/// A declined result, carrying what a caller branches on and the sentence a person reads.
///
/// `message` has no public constructor path of its own: every way of building a `Refusal`
/// generates it from the other fields, so two surfaces cannot describe one failure two ways.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Refusal {
    pub code: RefusalCode,
    /// The rule that declined, named as the registry names it.
    pub method_id: String,
    /// `weighing`, `onset`, `takeoff`, or None when the refusal is not about a landmark.
    pub slot: Option<String>,
    pub parameter: Option<String>,
    pub value: Option<f64>,
    /// Everything else the rule read while declining. Ordered, so the sentence is stable
    /// across runs.
    pub detail: BTreeMap<String, f64>,
    /// What the caller could have asked for instead.
    pub available: Vec<String>,
    message: String,
}

impl Refusal {
    fn build(
        code: RefusalCode,
        method_id: impl Into<String>,
        parameter: Option<String>,
        value: Option<f64>,
        detail: BTreeMap<String, f64>,
        available: Vec<String>,
    ) -> Self {
        let mut refusal = Self {
            code,
            method_id: method_id.into(),
            slot: None,
            parameter,
            value,
            detail,
            available,
            message: String::new(),
        };
        refusal.regenerate();
        refusal
    }

    fn regenerate(&mut self) {
        self.message = sentence(
            self.code,
            &self.method_id,
            self.slot.as_deref(),
            self.parameter.as_deref(),
            self.value,
            &self.detail,
            &self.available,
        );
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn exit_code(&self) -> i32 {
        exit_code(self.code)
    }

    /// Names the slot the refusal happened in. Kept separate from construction because a
    /// core rule does not know which step a caller bound it to.
    pub fn in_slot(mut self, slot: impl Into<String>) -> Self {
        self.slot = Some(slot.into());
        self.regenerate();
        self
    }

    /// Restamps the id the refusal is reported under, and regenerates the sentence so the
    /// two cannot disagree.
    ///
    /// One rule reached under two ids used to succeed under the id that resolves and
    /// decline under one that does not, so which name a caller saw depended on whether the
    /// rule worked.
    pub fn under(mut self, method_id: impl Into<String>) -> Self {
        self.method_id = method_id.into();
        self.regenerate();
        self
    }

    pub fn no_crossing(
        method_id: impl Into<String>,
        parameter: impl Into<String>,
        value: f64,
        search_bound_seconds: f64,
    ) -> Self {
        Self::build(
            RefusalCode::NoCrossing,
            method_id,
            Some(parameter.into()),
            Some(value),
            BTreeMap::from([("search_bound_seconds".to_string(), search_bound_seconds)]),
            Vec::new(),
        )
    }

    pub fn collapsed_band(
        method_id: impl Into<String>,
        parameter: impl Into<String>,
        value: f64,
        dispersion_newtons: f64,
        threshold_newtons: f64,
    ) -> Self {
        Self::build(
            RefusalCode::CollapsedBand,
            method_id,
            Some(parameter.into()),
            Some(value),
            BTreeMap::from([
                ("dispersion_newtons".to_string(), dispersion_newtons),
                ("threshold_newtons".to_string(), threshold_newtons),
            ]),
            Vec::new(),
        )
    }

    pub fn empty_trace(method_id: impl Into<String>) -> Self {
        Self::build(
            RefusalCode::TraceTooShort,
            method_id,
            None,
            None,
            BTreeMap::from([("sample_count".to_string(), 0.0)]),
            Vec::new(),
        )
    }

    pub fn epoch_does_not_fit(
        method_id: impl Into<String>,
        requested_seconds: f64,
        start_seconds: f64,
        available_seconds: f64,
    ) -> Self {
        Self::build(
            RefusalCode::TraceTooShort,
            method_id,
            None,
            None,
            BTreeMap::from([
                ("requested_seconds".to_string(), requested_seconds),
                ("start_seconds".to_string(), start_seconds),
                ("available_seconds".to_string(), available_seconds),
            ]),
            Vec::new(),
        )
    }

    pub fn parameter_not_finite(
        method_id: impl Into<String>,
        parameter: impl Into<String>,
        value: f64,
    ) -> Self {
        Self::build(
            RefusalCode::ParameterNotFinite,
            method_id,
            Some(parameter.into()),
            Some(value),
            BTreeMap::new(),
            Vec::new(),
        )
    }

    pub fn method_not_implemented(
        method_id: impl Into<String>,
        slot: impl Into<String>,
        available: Vec<String>,
    ) -> Self {
        let slot = slot.into();
        Self::build(
            RefusalCode::MethodNotImplemented,
            method_id,
            None,
            None,
            BTreeMap::new(),
            available,
        )
        .in_slot(slot)
    }

    pub fn unknown_parameter(
        method_id: impl Into<String>,
        parameter: impl Into<String>,
        available: Vec<String>,
    ) -> Self {
        Self::build(
            RefusalCode::UnknownParameter,
            method_id,
            Some(parameter.into()),
            None,
            BTreeMap::new(),
            available,
        )
    }

    pub fn column_not_found(column: impl Into<String>, available: Vec<String>) -> Self {
        Self::build(
            RefusalCode::ColumnNotFound,
            "",
            Some(column.into()),
            None,
            BTreeMap::new(),
            available,
        )
    }

    pub fn sentinel_convention_unknown(
        convention: impl Into<String>,
        available: Vec<String>,
    ) -> Self {
        Self::build(
            RefusalCode::SentinelConventionUnknown,
            "",
            Some(convention.into()),
            None,
            BTreeMap::new(),
            available,
        )
    }

    pub fn registry_invalid(detail: impl Into<String>) -> Self {
        Self::build(
            RefusalCode::RegistryInvalid,
            "",
            Some(detail.into()),
            None,
            BTreeMap::new(),
            Vec::new(),
        )
    }

    /// A result was asked for while a choice the registry forces is still open. `available`
    /// carries the constructs whose choice is outstanding.
    pub fn decision_not_made(artifact: impl Into<String>, outstanding: Vec<String>) -> Self {
        Self::build(
            RefusalCode::DecisionNotMade,
            "",
            Some(artifact.into()),
            None,
            BTreeMap::new(),
            outstanding,
        )
    }
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for Refusal {}

/// The one place a refusal becomes a sentence.
fn sentence(
    code: RefusalCode,
    method_id: &str,
    slot: Option<&str>,
    parameter: Option<&str>,
    value: Option<f64>,
    detail: &BTreeMap<String, f64>,
    available: &[String],
) -> String {
    let named = |key: &str| detail.get(key).copied().unwrap_or(f64::NAN);
    let subject = match (parameter, value) {
        (Some(name), Some(number)) => format!("{method_id}({name} = {number})"),
        _ => method_id.to_string(),
    };
    match code {
        RefusalCode::NoCrossing => format!(
            "{subject} found no crossing within the search bound of {} s",
            named("search_bound_seconds")
        ),
        RefusalCode::CollapsedBand => format!(
            "{subject} has no band to search: dispersion is {} N and the threshold falls at {} N",
            named("dispersion_newtons"),
            named("threshold_newtons")
        ),
        RefusalCode::TraceTooShort if detail.contains_key("requested_seconds") => format!(
            "weighing epoch of {} s starting at {} s does not fit in a trace of {} s",
            named("requested_seconds"),
            named("start_seconds"),
            named("available_seconds")
        ),
        RefusalCode::TraceTooShort => "trace is empty".to_string(),
        RefusalCode::ParameterNotFinite => format!(
            "{} must be positive, got {}",
            parameter.unwrap_or("the parameter"),
            value.unwrap_or(f64::NAN)
        ),
        RefusalCode::MethodNotImplemented => match slot {
            Some(step) => format!(
                "'{method_id}' was passed as the {step} method, and the rules available for that step are {available:?}"
            ),
            None => format!(
                "'{method_id}' has no rule behind it, and the rules available are {available:?}"
            ),
        },
        RefusalCode::UnknownParameter => format!(
            "{method_id} does not read {}, and the names it reads are {available:?}",
            parameter.unwrap_or("that name")
        ),
        RefusalCode::ColumnNotFound => format!(
            "no column named {}, and the file carries {available:?}",
            parameter.unwrap_or("that")
        ),
        RefusalCode::SentinelConventionUnknown => format!(
            "{} is not a sentinel convention this reader applies, and it applies {available:?}",
            parameter.unwrap_or("that")
        ),
        RefusalCode::RegistryInvalid => format!(
            "the registry does not load: {}",
            parameter.unwrap_or("no detail")
        ),
        RefusalCode::DecisionNotMade => format!(
            "{} states every choice behind a number, and {available:?} {} still open",
            parameter.unwrap_or("this artifact"),
            if available.len() == 1 { "is" } else { "are" }
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ten_codes_each_carry_an_exit_status() {
        let codes = [
            (RefusalCode::NoCrossing, 65),
            (RefusalCode::CollapsedBand, 65),
            (RefusalCode::TraceTooShort, 65),
            (RefusalCode::ColumnNotFound, 65),
            (RefusalCode::MethodNotImplemented, 64),
            (RefusalCode::UnknownParameter, 64),
            (RefusalCode::ParameterNotFinite, 64),
            (RefusalCode::SentinelConventionUnknown, 64),
            (RefusalCode::DecisionNotMade, 64),
            (RefusalCode::RegistryInvalid, 78),
        ];
        assert_eq!(codes.len(), 10);
        for (code, expected) in codes {
            assert_eq!(exit_code(code), expected, "{code:?}");
        }
    }

    #[test]
    fn the_wire_spelling_of_every_code_is_snake_case() {
        let spellings = [
            (RefusalCode::NoCrossing, "\"no_crossing\""),
            (RefusalCode::CollapsedBand, "\"collapsed_band\""),
            (
                RefusalCode::MethodNotImplemented,
                "\"method_not_implemented\"",
            ),
            (RefusalCode::UnknownParameter, "\"unknown_parameter\""),
            (RefusalCode::ParameterNotFinite, "\"parameter_not_finite\""),
            (RefusalCode::TraceTooShort, "\"trace_too_short\""),
            (RefusalCode::ColumnNotFound, "\"column_not_found\""),
            (
                RefusalCode::SentinelConventionUnknown,
                "\"sentinel_convention_unknown\"",
            ),
            (RefusalCode::RegistryInvalid, "\"registry_invalid\""),
            (RefusalCode::DecisionNotMade, "\"decision_not_made\""),
        ];
        for (code, expected) in spellings {
            assert_eq!(serde_json::to_string(&code).unwrap(), expected);
        }
    }

    #[test]
    fn the_shipped_sentences_are_reproduced_unchanged() {
        assert_eq!(
            Refusal::no_crossing("onset.threshold.noise_relative", "k", 5.0, 2.5).message(),
            "onset.threshold.noise_relative(k = 5) found no crossing within the search bound of 2.5 s"
        );
        assert_eq!(
            Refusal::collapsed_band("onset.threshold.noise_relative", "k", 5.0, 0.4, 812.1)
                .message(),
            "onset.threshold.noise_relative(k = 5) has no band to search: dispersion is 0.4 N and the threshold falls at 812.1 N"
        );
        assert_eq!(
            Refusal::epoch_does_not_fit("bwepoch.fixed_window", 2.0, 1.5, 3.0).message(),
            "weighing epoch of 2 s starting at 1.5 s does not fit in a trace of 3 s"
        );
        assert_eq!(
            Refusal::empty_trace("bwepoch.fixed_window").message(),
            "trace is empty"
        );
    }

    #[test]
    fn restamping_the_id_rewrites_the_sentence_rather_than_leaving_the_old_one() {
        let refused = Refusal::no_crossing("onset.threshold.percent_bodyweight", "k", 5.0, 2.5)
            .under("onset.threshold.relative_to_system_weight");
        assert_eq!(
            refused.method_id,
            "onset.threshold.relative_to_system_weight"
        );
        assert!(refused
            .message()
            .starts_with("onset.threshold.relative_to_system_weight(k = 5)"));
        assert!(!refused.message().contains("percent_bodyweight"));
    }

    #[test]
    fn a_refusal_round_trips_through_its_wire_form() {
        let refused =
            Refusal::no_crossing("onset.threshold.noise_relative", "k", 5.0, 2.5).in_slot("onset");
        let wire = serde_json::to_string(&refused).unwrap();
        assert_eq!(
            serde_json::from_str::<Refusal>(&wire).unwrap(),
            refused,
            "{wire}"
        );
        assert!(wire.contains("\"slot\":\"onset\""));
        assert!(wire.contains("\"message\":"));
    }

    #[test]
    fn a_decision_left_open_names_what_is_outstanding() {
        let refused = Refusal::decision_not_made(
            "A methods paragraph",
            vec!["movement_onset".to_string(), "system_weight".to_string()],
        );
        assert_eq!(refused.code, RefusalCode::DecisionNotMade);
        assert_eq!(refused.exit_code(), 64);
        assert!(refused.message().contains("movement_onset"));
    }
}
