//! `jumpheight.dj.mcmahon_correction_factor`: the arrival recovered from standing still.
//!
//! A drop jump starts with the athlete already falling, and an impulse-momentum integration
//! starts from rest, so something has to supply the velocity the athlete arrived with. The
//! deprecated route substitutes the box height, which the centre of mass does not fall the
//! whole of: the registry records 0.7 to 12.5 cm of disagreement, growing with the box. This
//! route measures the arrival instead of assuming it, and the registry records no bias against
//! the two-plate criterion that needs a second force plate to do the same job.
//!
//! The athlete stands still for the last stretch of the recording, so their true velocity there
//! is zero. Integrating the net force forward from the arrival with zero written at the arrival
//! gives a series that is wrong by exactly the arrival velocity, everywhere, and what that
//! series reads over the standing period is therefore the arrival velocity with its sign
//! reversed.
//!
//! The source seeds the integration from the box height and then corrects it by the same
//! comparison. The seed cancels: shifting every sample of the series by `v` moves the mean of
//! the standing period by `v` too, so the correction returns the same arrival velocity whatever
//! was seeded. This rule therefore takes no box height, because a name on the record that
//! cannot move the number is the fault this software exists to remove, and
//! `jumpheight.dj.box_height_as_drop_height` is the entry for a reader who wants the box.
//!
//! The same cancellation reaches the instant the integration starts at, which is worth knowing
//! before anyone reaches for the arrival to explain a number that moved: shifting the start
//! shifts the whole series by a constant and the correction removes it, so the height is the
//! same wherever the integration began. What the arrival decides is whether the rule runs,
//! because an arrival at or after takeoff leaves the integration no interval, and that is the
//! difference between a drop jump and a jump begun from standing.
//!
//! The standing period is the weighing window, as it is in the source, which weighs the athlete
//! over the final second and reads the velocity mean over that same second. So the window is
//! the weighing rule's choice and arrives on the record under that rule's own id, rather than
//! under a second name this entry would have to publish.

use plateforce_core::{
    arrival_velocity_from_final_standing_period_meters_per_second,
    centre_of_mass_velocity_meters_per_second, jump_height_from_takeoff_velocity,
    IntegrationAnchor, IntegrationDirection, IntegrationSpec, IntegrationStart, QuadratureRule,
    Refusal,
};

use crate::binding::TAKEOFF_CONSTRUCT;
use crate::derived::{DerivedContext, DerivedOutcome, DerivedRule};
use crate::request::MethodChoice;
use crate::resolution::{Resolution, RuleRefusal};
use crate::response::Quantity;
use crate::slots::landing;

pub const ID: &str = "jumpheight.dj.mcmahon_correction_factor";

pub const QUANTITIES: &[Quantity] = &[Quantity {
    key: super::KEY,
    label: "Jump height, takeoff frame",
    unit: "meters",
    computed_by: Some(ID),
    produced_by_construct: None,
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

    let Some(takeoff_index) = context.takeoff_index() else {
        return DerivedOutcome::declined(
            resolved.finish(),
            context.unavailable(ID, &[TAKEOFF_CONSTRUCT]),
        );
    };
    let Some(arrival_index) = landing::placed(context) else {
        return DerivedOutcome::declined(
            resolved.finish(),
            context.unavailable(ID, &[landing::CONSTRUCT]),
        );
    };

    // The integration runs from the arrival to takeoff, so an arrival at or after takeoff
    // leaves it nothing to run over. That is what a landing rule reports on a recording with
    // no drop in it: the return to the plate after the jump is the only landing there is.
    //
    // Reported as a search that found nothing rather than as a value that was not accepted,
    // because nobody stated the landing: a rule placed it, and what is wrong is that this
    // recording holds no arrival before takeoff. A code naming a value would send a reader to
    // change one they never typed.
    let epoch = context.epoch();
    if arrival_index >= takeoff_index {
        return DerivedOutcome::declined(
            resolved.finish(),
            RuleRefusal::Refused(Box::new(Refusal::no_crossing(
                ID,
                landing::PLACED,
                context.trial.time_at(arrival_index),
                context.trial.time_at(takeoff_index),
            ))),
        );
    }
    // Body weight and the velocity reference are one window in the source, and it is the one
    // the athlete stands still in after landing. A weighing window taken before the jump
    // weighs an empty plate on a drop jump and reads a velocity the athlete was moving
    // through, so the number it would produce is wrong in a way no reader could see.
    if epoch.start_index <= takeoff_index {
        return DerivedOutcome::declined(
            resolved.finish(),
            RuleRefusal::Refused(Box::new(Refusal::value_not_accepted(
                ID,
                "weighing_window_start_seconds",
                context.trial.time_at(epoch.start_index),
                vec![format!(
                    "a window inside the standing period after landing, so after the takeoff at \
                     {:.4} s, which is where this rule's body weight and velocity reference come \
                     from",
                    context.trial.time_at(takeoff_index)
                )],
            ))),
        );
    }

    // Zero at the arrival, which is the whole trick: the series is then wrong by the arrival
    // velocity everywhere, and the standing period says by how much.
    let spec = IntegrationSpec {
        quadrature: QuadratureRule::Trapezoid,
        direction: IntegrationDirection::Forward,
        start: IntegrationStart::DetectedTouchdown {
            index: arrival_index,
        },
        anchor: IntegrationAnchor::SinglePoint {
            index: arrival_index,
        },
    };
    context.rests_on(super::KEY, &spec.method_ids());
    let gravity = context.gravity_behind(Some(super::KEY));
    let velocity = centre_of_mass_velocity_meters_per_second(context.trial, epoch, &spec, gravity);
    let bound = resolved.finish();

    // The series carries one sample per sample of the recording, so both reads below are asking
    // whether the recording holds the samples this rule averages over. Refused together and in
    // the standing period's own terms, because the window is what a reader moves either way.
    let standing_period = epoch.start_index..epoch.end_index + 1;
    let reached = arrival_velocity_from_final_standing_period_meters_per_second(
        velocity.meters_per_second(),
        standing_period.clone(),
    );
    let (Some(arrival_velocity), Some(change_from_arrival)) = (reached, velocity.at(takeoff_index))
    else {
        return DerivedOutcome::declined(
            bound,
            RuleRefusal::Refused(Box::new(Refusal::epoch_does_not_fit(
                ID,
                standing_period.len() as f64 * context.trial.sample_interval_seconds(),
                context.trial.time_at(standing_period.start),
                context.trial.duration_seconds(),
            ))),
        );
    };
    let takeoff_velocity = change_from_arrival + arrival_velocity;

    // A takeoff velocity at or below zero says the athlete was still descending at the instant
    // the takeoff rule placed, which no jump does. Squaring it would report the descent as a
    // height, quietly, so the refusal names the two instants the interval ran between.
    if takeoff_velocity <= 0.0 {
        return DerivedOutcome::declined(
            bound,
            RuleRefusal::Refused(Box::new(Refusal::value_not_accepted(
                ID,
                landing::PLACED,
                context.trial.time_at(arrival_index),
                vec![format!(
                    "an arrival the contact phase to the takeoff at {:.4} s reverses, rather than \
                     one leaving the athlete travelling at {takeoff_velocity:.4} m/s there",
                    context.trial.time_at(takeoff_index)
                )],
            ))),
        );
    }

    DerivedOutcome {
        values: vec![(
            super::KEY,
            Some(jump_height_from_takeoff_velocity(takeoff_velocity, gravity)),
        )],
        placed: Vec::new(),
        bound,
        refusal: None,
    }
}
