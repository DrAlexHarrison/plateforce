//! `impulse.net_vertical.as_performance_determinant`: force less system weight, integrated
//! from onset to takeoff, and the velocity that impulse gave.
//!
//! The divisor is the mass the impulse accelerated, which is the athlete together with
//! anything they carry, so it is the weighed system mass rather than body mass. Under an
//! external load the two differ by the load, and dividing by body mass alone would attribute
//! the bar's momentum to the athlete.
//!
//! The two numbers rest on different things. The impulse is integrated directly over the
//! interval the landmarks bound. The velocity is read off the centre-of-mass series, whose
//! four integration entries move it: the two published starts give different velocities from
//! one recording.

use plateforce_core::takeoff_velocity_meters_per_second;

use crate::binding::{ONSET_CONSTRUCT, TAKEOFF_CONSTRUCT};
use crate::centre_of_mass;
use crate::derived::{DerivedContext, DerivedOutcome, DerivedRule};
use crate::request::MethodChoice;
use crate::resolution::Resolution;
use crate::response::Quantity;

pub const ID: &str = "impulse.net_vertical.as_performance_determinant";

pub const QUANTITIES: &[Quantity] = &[
    Quantity {
        key: super::KEY,
        label: "Net impulse",
        unit: "newton_seconds",
        computed_by: Some(ID),
        produced_by_construct: None,
    },
    Quantity {
        key: super::VELOCITY_KEY,
        label: "Takeoff velocity",
        unit: "meters_per_second",
        computed_by: Some(ID),
        produced_by_construct: None,
    },
];

pub const RULE: DerivedRule = compute;

fn compute(
    context: &DerivedContext,
    choice: &MethodChoice,
    _warnings: &mut Vec<String>,
) -> DerivedOutcome {
    let mut resolved = Resolution::over(
        &choice.parameters,
        &choice.options,
        choice.declared.of_entry(ID),
        choice.claims(),
    );

    let Some(landmarks) = context.landmarks() else {
        return DerivedOutcome::declined(
            resolved.finish(),
            context.unavailable(ID, &[ONSET_CONSTRUCT, TAKEOFF_CONSTRUCT]),
        );
    };

    // Named against the velocity alone. The impulse below is integrated over the interval
    // directly rather than read off the series these four built, so a chain naming them on it
    // would cite choices that number never passed through.
    let spec = centre_of_mass::spec_anchored_at(landmarks.onset_index);
    context.rests_on(super::VELOCITY_KEY, &spec.method_ids());
    centre_of_mass::record_choices(&mut resolved, landmarks.onset_index);

    let impulse_newton_seconds = context.trial.integrate_offset_newton_seconds(
        landmarks.onset_index,
        landmarks.takeoff_index,
        context.epoch().system_weight_newtons,
    );
    let velocity = takeoff_velocity_meters_per_second(
        context.trial,
        context.epoch(),
        &landmarks,
        context.gravity_behind(Some(super::VELOCITY_KEY)),
    );

    DerivedOutcome {
        values: vec![
            (super::KEY, Some(impulse_newton_seconds)),
            (super::VELOCITY_KEY, Some(velocity)),
        ],
        placed: Vec::new(),
        bound: resolved.finish(),
        refusal: None,
    }
}
