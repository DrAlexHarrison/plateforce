//! `filter.none`: the recording as it was digitised.
//!
//! The arithmetic is the identity and the whole of the rule is the declaration. The registry
//! entry's own rationale is the specification: a tool states its filter even when the answer
//! is none, because an undeclared no-filter is an absence rather than a choice, and a reader
//! cannot tell an absence from a filter somebody forgot to write down.

use plateforce_core::provenance::ParameterSource;
use plateforce_core::Trial;

use crate::conditioning::ConditioningOutcome;
use crate::request::MethodChoice;
use crate::resolution::Resolution;

pub const ID: &str = "filter.none";

pub(crate) fn apply(
    _trial: &Trial,
    choice: &MethodChoice,
    _warnings: &mut Vec<String>,
) -> ConditioningOutcome {
    let mut resolved = Resolution::over(&choice.parameters, &choice.options, choice.claims());
    // Recorded as a value this rule read, so the fingerprint carries the answer rather than
    // carrying the rule's name over an empty binding.
    resolved.record("passband_edge", "none".to_string(), ParameterSource::Assumed);
    ConditioningOutcome::unchanged(resolved.finish())
}
