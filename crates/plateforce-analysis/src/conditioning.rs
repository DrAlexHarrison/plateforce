//! What was done to the signal before anything was measured on it.
//!
//! The landmark rules read a series, and which series that is is a choice. A tool that
//! filters and does not say so reports a number nobody can reproduce, and a tool that does
//! not filter and does not say so reports one nobody can tell from the first. So this phase
//! runs whether or not a caller asked for anything, and its answer is on the record either
//! way.
//!
//! It runs before the spine because the spine's thresholds are scaled by what it produces.
//! Applying one published filter narrowed the quiet-epoch spread on one recording from
//! 1.41 N to 0.95 N, which narrowed the onset band from 7.03 N to 4.77 N, which moved the
//! onset 710 ms earlier onto a postural sway. A conditioning rule that ran after the
//! landmarks would leave the record saying they were placed on a signal they were not.

use plateforce_core::Trial;

use crate::request::MethodChoice;
use crate::resolution::{BoundValues, RuleRefusal};

/// What a conditioning rule produces.
pub struct ConditioningOutcome {
    /// The conditioned series. `None` where the rule is the identity and the recording is
    /// used as it was digitised, which is a rule with an answer rather than an absent step.
    pub force_newtons: Option<Vec<f64>>,
    pub bound: BoundValues,
    pub refusal: Option<RuleRefusal>,
}

impl ConditioningOutcome {
    /// The recording as it stands, under the rule that says so.
    pub fn unchanged(bound: BoundValues) -> Self {
        Self {
            force_newtons: None,
            bound,
            refusal: None,
        }
    }
}

/// A rule that conditions the signal the landmark rules then read.
///
/// Takes the trial rather than a context, because nothing has been resolved yet: this phase
/// is what the rest of the analysis is resolved against.
pub type ConditioningRule = fn(&Trial, &MethodChoice, &mut Vec<String>) -> ConditioningOutcome;
