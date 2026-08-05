//! `jumpheight.flight_phase_displacement.vald_impdis`: the highest the displacement curve
//! reaches between takeoff and landing.
//!
//! The integration runs from quiet standing, so the curve this takes a maximum of is measured
//! from the standing position while the window it searches is the flight. The rule states no
//! frame for the number that comes out.

use plateforce_core::Refusal;

use crate::binding::{ONSET_CONSTRUCT, TAKEOFF_CONSTRUCT};
use crate::centre_of_mass;
use crate::derived::{DerivedContext, DerivedOutcome, DerivedRule};
use crate::request::MethodChoice;
use crate::resolution::{Resolution, RuleRefusal};
use crate::response::Quantity;
use crate::slots::flight_time;

pub const ID: &str = "jumpheight.flight_phase_displacement.vald_impdis";

pub const QUANTITIES: &[Quantity] = &[Quantity {
    key: super::KEY,
    label: "Jump height, frame not stated",
    unit: "meters",
    computed_by: Some(ID),
}];

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
    let Some(touchdown_index) = context.touchdown_index() else {
        return DerivedOutcome::declined(
            resolved.finish(),
            RuleRefusal::Refused(Box::new(Refusal::required_parameter_unstated(
                ID,
                flight_time::TOUCHDOWN_FIELD,
            ))),
        );
    };

    let displacement = centre_of_mass::displacement(
        context.trial,
        context.epoch(),
        landmarks.onset_index,
        context.gravity_behind(Some(super::KEY)),
        &mut resolved,
    );
    let bound = resolved.finish();

    let end = touchdown_index.saturating_add(1).min(displacement.len());
    match plateforce_core::peak::maximum_over(displacement.meters(), landmarks.takeoff_index, end) {
        Ok(apex_meters) => DerivedOutcome {
            values: vec![(super::KEY, Some(apex_meters))],
            placed: Vec::new(),
            bound,
            refusal: None,
        },
        Err(_) => DerivedOutcome::declined(
            bound,
            RuleRefusal::Refused(Box::new(Refusal::span_selects_no_samples(
                ID,
                landmarks.takeoff_index,
                end,
            ))),
        ),
    }
}
