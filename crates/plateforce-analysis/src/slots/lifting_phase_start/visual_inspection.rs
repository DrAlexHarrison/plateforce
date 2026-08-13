//! `phase.lift.start.visual_inspection.dead_start`: the onset a reader placed by eye.
//!
//! The registry's text: the onset of each repetition is determined by visual inspection of
//! the force-time curve, and no algorithmic rule is stated. That is not a rule this build
//! cannot run. It is a rule whose input is a person, and the input already exists: a request
//! carries `manual_index` for exactly this, and `bwepoch.manual_placement` has been bound on
//! the same footing since the weighing window shipped.
//!
//! So the rule runs and asks for the one thing it lacks. An unstated instant is refused by
//! name rather than filled from a neighbouring rule, because a dead-start lift is the one
//! signal shape with no kinematic event to anchor to: a slow monotonic ramp, no
//! countermovement, and therefore no velocity zero crossing for any automatic rule to find.
//! Substituting one here would put a landmark under this entry's name that no eye placed.
//!
//! The instant travels in the record as this rule with a manual override rather than as a
//! rule that searched, so two readers who placed it differently produce two different
//! fingerprints. The registry calls this entry's sensitivity extreme and worse than anywhere
//! else in the corpus, and it is unquantified: a fixed-epoch impulse is by construction
//! maximally sensitive to where the epoch starts, and nobody has measured the spread between
//! two people eyeballing the same ramp.

use crate::derived::{DerivedContext, DerivedOutcome, DerivedRule};
use crate::request::MethodChoice;
use crate::resolution::{Resolution, RuleRefusal};
use crate::response::Quantity;

pub const ID: &str = "phase.lift.start.visual_inspection.dead_start";

/// What the request calls the instant a reader placed, so a refusal names the field a caller
/// has to fill rather than describing it.
const PLACED_INSTANT: &str = "manual_index";

pub const QUANTITIES: &[Quantity] = &[Quantity {
    key: super::KEY,
    label: "Start of the lift",
    unit: "seconds",
    computed_by: Some(ID),
    produced_by_construct: None,
}];

pub const RULE: DerivedRule = place;

fn place(
    context: &DerivedContext,
    choice: &MethodChoice,
    _warnings: &mut Vec<String>,
) -> DerivedOutcome {
    let resolved = Resolution::over(
        &choice.parameters,
        &choice.options,
        choice.declared.of_entry(ID),
        choice.claims(),
    );
    let bound = resolved.finish();

    let Some(index) = choice.manual_index else {
        return DerivedOutcome::declined(
            bound,
            RuleRefusal::Refused(Box::new(
                plateforce_core::Refusal::required_parameter_unstated(ID, PLACED_INSTANT),
            )),
        );
    };

    // An instant past the end of the recording is not an instant on this trace. Refused
    // naming both, rather than clamped to the last sample, because a clamped landmark reads
    // as the reader's answer and is the file's.
    if index >= context.trial.len() {
        return DerivedOutcome::declined(
            bound,
            RuleRefusal::Refused(Box::new(plateforce_core::Refusal::span_selects_no_samples(
                ID,
                index,
                context.trial.len(),
            ))),
        );
    }

    crate::boundaries::placed_outcome(context, super::KEY, super::PLACED, Some(index), bound)
}
