//! `phase.landing_end.zero_com_velocity`: the landing ends where the descent stops.
//!
//! The registry's text: the first frame where centre of mass velocity, integrated from
//! landing acceleration with the negative takeoff velocity as its initial value, reaches
//! zero. The initial value is the whole of what makes this quantity what it is, so the core
//! checks the anchor rather than trusting the caller: a landing velocity integrated from
//! anywhere else is a different quantity that happens to share a unit.
//!
//! The registry records that one shipped implementation raises an index error when a trial
//! ends soon after touchdown, and that a short trial there produces no landing metrics at all
//! rather than degraded ones. This reports which of the two happened: a landing that settled
//! inside the recording, or a recording that ran out with the centre of mass still
//! descending, with the velocity it was still moving at.
//!
//! `integration.anchor.*` carries no entry for a stated non-zero start value, so the anchor
//! records this rule's own id as the thing that chose it rather than crediting an anchor
//! entry that did not.

use plateforce_core::phases::{landing_end_by_zero_com_velocity, LandingEnd};
use plateforce_core::series::{
    centre_of_mass_velocity_meters_per_second, IntegrationAnchor, IntegrationDirection,
    IntegrationSpec, IntegrationStart, QuadratureRule,
};
use plateforce_core::takeoff_velocity_meters_per_second;

use crate::binding::{ONSET_CONSTRUCT, TAKEOFF_CONSTRUCT};
use crate::boundaries;
use crate::derived::{DerivedContext, DerivedOutcome, DerivedRule};
use crate::request::MethodChoice;
use crate::resolution::Resolution;
use crate::response::Quantity;
use crate::slots::landing;

pub const ID: &str = "phase.landing_end.zero_com_velocity";

pub const QUANTITIES: &[Quantity] = &[Quantity {
    key: super::KEY,
    label: "End of the landing",
    unit: "seconds",
    computed_by: Some(ID),
    produced_by_construct: None,
}];

pub const RULE: DerivedRule = place;

fn place(
    context: &DerivedContext,
    choice: &MethodChoice,
    warnings: &mut Vec<String>,
) -> DerivedOutcome {
    let mut resolved = Resolution::over(
        &choice.parameters,
        &choice.options,
        choice.declared.of_entry(ID),
        choice.claims(),
    );

    let Some(landmarks) = context.landmarks() else {
        let missing = boundaries::absent(context, &[ONSET_CONSTRUCT, TAKEOFF_CONSTRUCT]);
        return DerivedOutcome::declined(resolved.finish(), context.unavailable(ID, &missing));
    };
    // The placed touchdown, not the fallback `landmarks` fills an unstated one with. This
    // rule is about the stretch after the athlete is back on the plate, and anchoring it at
    // the last sample of a recording nobody landed in would report a landing that never was.
    let Some(touchdown) = landing::placed(context).or_else(|| context.touchdown_index()) else {
        return DerivedOutcome::declined(
            resolved.finish(),
            context.unavailable(ID, &[landing::CONSTRUCT]),
        );
    };

    // `None`, because this boundary does not move with the gravity that scaled the series it
    // is read off. Acceleration is g(F - W)/W, so the integrated velocity scales by g, and
    // the anchor value is a takeoff velocity read off the same scaling; both terms carry the
    // factor, and scaling a series moves neither its zeros nor its extrema. Recorded against
    // the key, this boundary named a value that moves it by nothing, and every number
    // measured across it would have inherited a dependence it has not got. The guard measured
    // exactly that: the same sample at 9.80665 and at the second gravity, with the chain
    // saying otherwise.
    let gravity = context.gravity_behind(None);
    let takeoff_velocity =
        takeoff_velocity_meters_per_second(context.trial, context.epoch(), &landmarks, gravity);

    // The integral runs from the trial start and the constant is pinned at touchdown, which
    // is the same curve after touchdown as integrating the landing alone and keeps one
    // quadrature behind every series this build reports.
    let spec = IntegrationSpec {
        quadrature: QuadratureRule::Trapezoid,
        direction: IntegrationDirection::Forward,
        start: IntegrationStart::TrialStart,
        anchor: IntegrationAnchor::SinglePointAtValue {
            index: touchdown,
            value: -takeoff_velocity,
            stated_by_method_id: ID.to_string(),
        },
    };
    let velocity =
        centre_of_mass_velocity_meters_per_second(context.trial, context.epoch(), &spec, gravity);
    let [quadrature, direction, start, _anchor] = spec.method_ids();
    for (name, id) in [
        ("integration_rule", quadrature),
        ("integration_direction", direction),
        ("integration_start", start),
    ] {
        resolved.record(
            name,
            id.to_string(),
            plateforce_core::provenance::ParameterSource::Assumed,
        );
    }
    let bound = resolved.finish();

    match landing_end_by_zero_com_velocity(&velocity, touchdown) {
        LandingEnd::Settled { index } => {
            boundaries::placed_outcome(context, super::KEY, super::PLACED, Some(index), bound)
        }
        // Named rather than left empty. The number a reader wants does not exist here, and
        // the reason is the length of the recording rather than anything about the athlete.
        LandingEnd::RecordingEndsWhileStillMoving {
            velocity_meters_per_second,
            ..
        } => {
            warnings.push(format!(
                "{ID} reached the end of the recording with the centre of mass still descending at {velocity_meters_per_second:.4} m/s, so this recording holds no instant the landing ended at"
            ));
            boundaries::placed_outcome(context, super::KEY, super::PLACED, None, bound)
        }
        LandingEnd::NotAnchoredAtTouchdown { touchdown_index } => DerivedOutcome::declined(
            bound,
            crate::resolution::RuleRefusal::Refused(Box::new(
                plateforce_core::Refusal::span_selects_no_samples(
                    ID,
                    touchdown_index,
                    context.trial.len(),
                ),
            )),
        ),
    }
}
