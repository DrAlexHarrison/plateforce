//! `landing.threshold.absolute_force`: a rising edge stated in its own right.
//!
//! The threshold and the span come from the caller rather than from whatever placed takeoff,
//! so a threshold error at one edge of the flight phase stays at that edge. One surveyed tool
//! requires 250 ms at the falling edge and 15 ms here, a factor of about seventeen, because a
//! landing is a step change and a takeoff is the absence of one, and the two edges are not
//! equally exposed to a brief excursion.
//!
//! The search runs the whole recording and asks only that the plate was unloaded first, which
//! is what makes it independent of takeoff rather than merely differently thresholded. On a
//! countermovement jump the plate is first unloaded during flight, so the rule reads the
//! post-flight landing. On a drop jump the recording opens with the athlete still on the box
//! and the plate unloaded, so the same search reads the arrival, which is the instant the first
//! foot reaches the plate and the one a drop-jump integration has to start from.

use plateforce_core::landing_first_sustained_run;

use crate::boundaries;
use crate::derived::{DerivedContext, DerivedOutcome, DerivedRule};
use crate::request::MethodChoice;
use crate::resolution::{Resolution, RuleRefusal};
use crate::response::Quantity;

pub const ID: &str = "landing.threshold.absolute_force";

/// The registry publishes both, recovered from one implementation. A single published value
/// for the rising edge is not evidence that the two edges agree, which is the note the entry
/// carries and the reason this rule exists beside the tied one.
const THRESHOLD_DEFAULT_NEWTONS: f64 = 20.0;
const PERSISTENCE_DEFAULT_MILLISECONDS: f64 = 15.0;

pub const QUANTITIES: &[Quantity] = &[Quantity {
    key: super::KEY,
    label: "Landing",
    unit: "seconds",
    computed_by: Some(ID),
}];

pub const RULE: DerivedRule = place;

fn place(
    context: &DerivedContext,
    choice: &MethodChoice,
    _warnings: &mut Vec<String>,
) -> DerivedOutcome {
    let mut resolved = Resolution::over(&choice.parameters, &choice.options, choice.claims());
    let rate_hz = context.trial.sample_rate_hz();
    let threshold_newtons = resolved.number(super::THRESHOLD_PARAMETER, THRESHOLD_DEFAULT_NEWTONS);
    let minimum_contact_samples = resolved
        .milliseconds_as_samples(
            super::PERSISTENCE_PARAMETER,
            PERSISTENCE_DEFAULT_MILLISECONDS,
            rate_hz,
        )
        .max(1);

    // No landmark is read, which is the entry's claim rather than an oversight: this rule
    // searches the recording rather than the interval another rule bounded, so its chain names
    // the conditioning that produced the signal and nothing else.
    let found = landing_first_sustained_run(
        context.trial.force(),
        threshold_newtons,
        minimum_contact_samples,
        0,
        rate_hz,
        ID,
        super::THRESHOLD_PARAMETER,
    );
    let bound = resolved.finish();

    match found {
        Ok(index) => {
            boundaries::placed_outcome(context, super::KEY, super::PLACED, Some(index), bound)
        }
        Err(error) => DerivedOutcome::declined(bound, RuleRefusal::Trial(error)),
    }
}
