//! `jumpheight.frame`: which physical displacement a jump height denotes, stated rather than
//! computed.
//!
//! The entry carries no arithmetic. What it carries is the one choice a jump height cannot be
//! read without: apex above where the centre of mass sat at takeoff, or apex above where it
//! sat in quiet standing. Both are physically valid, they differ by the heel rise, and the
//! registry's verdict on this row is that it is the one jump-height choice a reader must not
//! be allowed to default through.
//!
//! So an unstated frame is refused rather than filled in.

use plateforce_core::Refusal;

use crate::derived::{DerivedContext, DerivedOutcome, DerivedRule};
use crate::request::MethodChoice;
use crate::resolution::{Resolution, RuleRefusal};
use crate::response::Quantity;

pub const ID: &str = "jumpheight.frame";

/// The entry's own name for the choice, and the two values it takes.
pub const FRAME_PARAMETER: &str = "frame";
pub const TAKEOFF: &str = "takeoff";
pub const STANDING: &str = "standing";

/// A declaration reports no number. It contributes the choice and the record of who made it,
/// which is what travels with every height computed beside it.
pub const QUANTITIES: &[Quantity] = &[];

pub const RULE: DerivedRule = compute;

fn compute(
    _context: &DerivedContext,
    choice: &MethodChoice,
    _warnings: &mut Vec<String>,
) -> DerivedOutcome {
    let mut resolved = Resolution::over(
        &choice.parameters,
        &choice.options,
        choice.declared.of_entry(ID),
        choice.claims(),
    );

    if resolved.stated_name(FRAME_PARAMETER).is_none() {
        return DerivedOutcome::declined(
            resolved.finish(),
            RuleRefusal::Refused(Box::new(Refusal::required_parameter_unstated(
                ID,
                FRAME_PARAMETER,
            ))),
        );
    }

    // The fallback is unreachable: the name was stated a line above. It is empty rather than
    // one of the two, so a reader of this rule cannot mistake either frame for a default.
    match resolved.enumerated(
        FRAME_PARAMETER,
        &[(TAKEOFF, TAKEOFF), (STANDING, STANDING)],
    ) {
        Ok(_) => DerivedOutcome {
            values: Vec::new(),
            placed: Vec::new(),
            bound: resolved.finish(),
            refusal: None,
        },
        Err(refusal) => DerivedOutcome::declined(resolved.finish(), refusal),
    }
}
