//! `power.instantaneous.force_x_velocity`: power at each instant is force multiplied by
//! velocity, under a stated force term and a stated sign.
//!
//! The two force terms differ by system weight times velocity, which at 2.5 m/s and 800 N is
//! 2000 W. Sign convention on phase-restricted power is unmanaged across the whole field,
//! including inside single papers by careful authors: one published table defines peak braking
//! power as the peak negative power and reports it negative for the countermovement jump and
//! positive for the drop jump in adjacent tables, and two commercial products differ on it by
//! an ordinary least products slope of -0.986, which is a pure sign flip. So neither is
//! defaulted and an unstated one is refused by name.
//!
//! The rule reports no number, because a series is not a value. What it contributes is the
//! record of what power meant for this analysis, and every peak, mean, integral and rate below
//! names this entry among the entries its number rests on.

use crate::binding::ONSET_CONSTRUCT;
use crate::derived::{DerivedContext, DerivedOutcome, DerivedRule};
use crate::request::MethodChoice;
use crate::resolution::Resolution;
use crate::response::Quantity;

pub const ID: &str = "power.instantaneous.force_x_velocity";

/// A series is not a value, so nothing is reported. The choices and the refusal are the whole
/// of what this rule produces.
pub const QUANTITIES: &[Quantity] = &[];

pub const RULE: DerivedRule = compute;

fn compute(
    context: &DerivedContext,
    choice: &MethodChoice,
    _warnings: &mut Vec<String>,
) -> DerivedOutcome {
    let mut resolved = Resolution::over(&choice.parameters, &choice.options, choice.claims());

    let Some(onset) = context.onset_index() else {
        return DerivedOutcome::declined(
            resolved.finish(),
            context.unavailable(ID, &[ONSET_CONSTRUCT]),
        );
    };

    // The series is formed rather than described, so a request whose velocity cannot be
    // integrated is refused here rather than by each rule that would have read it.
    let series = super::power_series(context, &mut resolved, ID, onset, None);
    let bound = resolved.finish();
    match series {
        Ok(_) => DerivedOutcome {
            values: Vec::new(),
            placed: Vec::new(),
            bound,
            refusal: None,
        },
        Err(refusal) => DerivedOutcome::declined(bound, refusal),
    }
}
