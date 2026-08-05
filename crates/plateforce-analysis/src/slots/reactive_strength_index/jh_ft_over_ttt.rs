//! `rsimod.jh_ft_over_ttt`: the flight-time jump height, over the time from onset to takeoff.
//!
//! The sibling of `rsimod.jh_tov_over_ttt` and its whole difference is the numerator. Both
//! divide by the same interval, so a result carries the construct's key and its entry id says
//! which height went into it.
//!
//! The size of that difference is measured and it depends on the movement. On a
//! countermovement jump the two are almost perfectly related, apparently because dividing by a
//! common time partially cancels the numerator bias. On a drop jump two vendors differ by 25.7
//! percent here while the unmodified index on the same trials agrees to 0.91 percent, because
//! neither vendor has to compute a height for that one. Removing a computed intermediate
//! removed the disagreement.
//!
//! The entry publishes no gravity of its own, so the projectile equation runs on the gravity
//! chosen for the analysis. `jumpheight.takeoff.flight_time` publishes one and prefers a
//! chosen value over it; that preference belongs to that entry's declaration rather than to
//! this one, and borrowing it would run a published constant this entry never published.

use plateforce_core::{
    jump_height_from_flight_time, reactive_strength_index_modified, time_to_takeoff_seconds,
    Refusal,
};

use crate::binding::{ONSET_CONSTRUCT, TAKEOFF_CONSTRUCT};
use crate::derived::{DerivedContext, DerivedOutcome, DerivedRule};
use crate::request::MethodChoice;
use crate::resolution::{Resolution, RuleRefusal};
use crate::response::Quantity;
use crate::slots::flight_time;

pub const ID: &str = "rsimod.jh_ft_over_ttt";

pub const QUANTITIES: &[Quantity] = &[Quantity {
    key: super::KEY,
    label: "RSI modified",
    unit: "meters_per_second",
    computed_by: Some(ID),
}];

pub const RULE: DerivedRule = compute;

fn compute(
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

    // Onset and takeoff bound the denominator; takeoff and the return to the plate bound the
    // numerator. So the landmark bundle is what this needs, and the touchdown is asked for
    // separately because the bundle fills an unstated one with the last sample of the
    // recording, which no rule placed.
    let Some(landmarks) = context.landmarks() else {
        return DerivedOutcome::declined(
            resolved.finish(),
            context.unavailable(ID, &[ONSET_CONSTRUCT, TAKEOFF_CONSTRUCT]),
        );
    };
    let Some(flight_seconds) = flight_time::seconds(context, landmarks.takeoff_index) else {
        return DerivedOutcome::declined(
            resolved.finish(),
            RuleRefusal::Refused(Box::new(Refusal::required_parameter_unstated(
                ID,
                flight_time::TOUCHDOWN_FIELD,
            ))),
        );
    };

    let gravity = context.gravity_behind(Some(super::KEY));
    let height_meters = jump_height_from_flight_time(flight_seconds, gravity);
    let seconds = time_to_takeoff_seconds(&landmarks, context.trial.sample_interval_seconds());

    DerivedOutcome {
        values: vec![(
            super::KEY,
            reactive_strength_index_modified(height_meters, seconds),
        )],
        placed: Vec::new(),
        bound: resolved.finish(),
        refusal: None,
    }
}
