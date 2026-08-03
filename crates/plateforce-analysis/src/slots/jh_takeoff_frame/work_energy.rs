//! `jumpheight.takeoff.work_energy`: net force integrated through the displacement it caused,
//! converted from kinetic energy at takeoff to a height.
//!
//! Force through a displacement and power through a time are one integral in continuous form,
//! so this reads the work the core already computes rather than integrating a second time.
//!
//! The entry's start point may be the beginning of the countermovement or the lowest point at
//! which velocity is again zero, and it states that the two are analytically identical because
//! kinetic energy is zero at both. So the start is the placed onset and no fork exists.

use plateforce_core::power::{
    instantaneous_power_watts, work_joules, DeclaredPhase, ForceTerm, PowerSignConvention,
};
use plateforce_core::Refusal;

use crate::binding::{ONSET_CONSTRUCT, TAKEOFF_CONSTRUCT};
use crate::centre_of_mass;
use crate::derived::{DerivedContext, DerivedOutcome, DerivedRule};
use crate::request::MethodChoice;
use crate::resolution::{Resolution, RuleRefusal};
use crate::response::Quantity;

pub const ID: &str = "jumpheight.takeoff.work_energy";

pub const QUANTITIES: &[Quantity] = &[Quantity {
    key: super::KEY,
    label: "Jump height, takeoff frame",
    unit: "meters",
    computed_by: Some(ID),
}];

pub const RULE: DerivedRule = compute;

fn compute(
    context: &DerivedContext,
    choice: &MethodChoice,
    _warnings: &mut Vec<String>,
) -> DerivedOutcome {
    let mut resolved = Resolution::over(&choice.parameters, &choice.options, choice.claims());

    let Some(landmarks) = context.landmarks() else {
        return DerivedOutcome::declined(
            resolved.finish(),
            context.unavailable(ID, &[ONSET_CONSTRUCT, TAKEOFF_CONSTRUCT]),
        );
    };
    let gravity = context.gravity_meters_per_second_squared;
    let velocity = centre_of_mass::velocity(
        context.trial,
        context.epoch,
        landmarks.onset_index,
        gravity,
        &mut resolved,
    );
    let bound = resolved.finish();

    // Net of system weight, because the work that changes kinetic energy is the work the
    // accelerating force did. Against the measured force the integral also counts holding the
    // athlete up, which is not energy the jump gained.
    let power = match instantaneous_power_watts(
        context.trial.force(),
        &velocity,
        context.epoch.system_weight_newtons,
        ForceTerm::NetOfSystemWeight,
        PowerSignConvention::UpwardPositive,
    ) {
        Ok(power) => power,
        Err(_) => {
            return DerivedOutcome::declined(
                bound,
                RuleRefusal::Refused(Box::new(Refusal::span_selects_no_samples(
                    ID,
                    landmarks.onset_index,
                    landmarks.takeoff_index,
                ))),
            )
        }
    };

    let phase = DeclaredPhase {
        first_index: landmarks.onset_index,
        last_index: centre_of_mass::last_sample_in_contact(landmarks.takeoff_index),
        method_id: ID.to_string(),
    };
    match work_joules(&power, &phase, context.trial.sample_interval_seconds()) {
        Ok(joules) => {
            let mass = context.epoch.system_mass_kilograms(gravity);
            DerivedOutcome {
                values: vec![(super::KEY, Some(joules / (mass * gravity)))],
                placed: Vec::new(),
                bound,
                refusal: None,
            }
        }
        Err(_) => DerivedOutcome::declined(
            bound,
            RuleRefusal::Refused(Box::new(Refusal::span_selects_no_samples(
                ID,
                phase.first_index,
                phase.last_index,
            ))),
        ),
    }
}
