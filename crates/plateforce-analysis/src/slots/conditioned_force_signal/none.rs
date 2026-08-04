//! `filter.none`: the recording as it was digitised.
//!
//! The arithmetic is the identity and the whole of the rule is the declaration. The registry
//! entry's own rationale is the specification: a tool states its filter even when the answer
//! is none, because an undeclared no-filter is an absence rather than a choice, and a reader
//! cannot tell an absence from a filter somebody forgot to write down.

use plateforce_core::Trial;

use crate::conditioning::ConditioningOutcome;
use crate::request::MethodChoice;
use crate::resolution::Resolution;

pub const ID: &str = "filter.none";

/// The edge this rule reads, and the whole of what it declares. `registry/methods/acquisition.toml`
/// files it as an enumeration and names one value for it, which is this one.
const PASSBAND_EDGE: &str = "passband_edge";
const NO_PASSBAND_EDGE: &str = "none";

pub(crate) fn apply(
    _trial: &Trial,
    choice: &MethodChoice,
    _warnings: &mut Vec<String>,
) -> ConditioningOutcome {
    let mut resolved = Resolution::over(&choice.parameters, &choice.options, choice.claims());
    // Recorded as a value this rule read, so the fingerprint carries the answer rather than
    // carrying the rule's name over an empty binding.
    //
    // Through `entailed`, because picking this rule is what settles the edge. A caller who
    // states the edge it takes keeps their signature on it; one who states a different edge
    // is refused by name rather than reading `none` in a record they did not ask for; one who
    // states nothing gets the rule's own value, marked as the rule's. Before this the value
    // was recorded as assumed whatever the caller said, and a stated edge went to
    // `unread_parameters` under a record reading `none, assumed`.
    if let Err(refusal) = resolved.entailed(ID, PASSBAND_EDGE, NO_PASSBAND_EDGE) {
        return ConditioningOutcome::refused(resolved.finish(), refusal);
    }
    ConditioningOutcome::unchanged(resolved.finish())
}
