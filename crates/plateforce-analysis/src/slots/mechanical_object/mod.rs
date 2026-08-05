//! Which mechanical object a loaded-lift quantity describes: the bar, the athlete, or both
//! together.
//!
//! The headline number of the loaded domain. On one deadlift dataset, eight men, same trials,
//! same session, peak power occurs at 50 percent of one-repetition maximum for the bar, 30
//! percent for the body and 70 percent for the system. Not a disagreement about data, a
//! threshold or a filter: a disagreement about what the number is a number about.
//!
//! The registry files this construct behind a barbell and a loaded lift, which is honest about
//! the operator's recordings: on an unloaded jump the three objects coincide and the axis does
//! not exist. It is not honest about this build, which reads the choice, resolves it to a mass,
//! and refuses by name when the athlete's own mass is unstated. That refusal is the software
//! asking for the evidence it lacks, and the mass it returns is what a subtrahend and a divisor
//! are both built from.

pub mod computed_on_object;

use plateforce_core::Refusal;

use crate::derived::DerivedContext;
use crate::request::BODY_MASS_GLOBAL;
use crate::resolution::{Resolution, RuleRefusal};

/// The construct id, as `registry/constructs.toml` declares it.
pub const CONSTRUCT: &str = "mechanical_object";

/// The key the declaration reports the object's mass under.
pub const KEY: &str = "mechanical_object_mass_kilograms";

/// The three objects, as the registry's two entries name them between them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Object {
    Barbell,
    Body,
    System,
}

/// The mass of one named object, from the weighed system and the athlete's stated mass.
///
/// One home, because two entries carry this choice under two names. `declaration.computed_on_object`
/// asks which object a quantity describes and `normalise.denominator` asks which mass it is
/// divided by, and the two are the same three masses. Computing them in both places would let
/// a record say a quantity describes the bar while dividing it by something else.
///
/// The bar is what the plate weighed less what the athlete stated, so an unloaded jump answers
/// near zero, which is the true answer rather than a refusal. A stated mass heavier than the
/// weighed system is refused: it is a bar of negative mass, and dividing by it would report a
/// sign flip as a measurement.
pub(crate) fn mass_kilograms(
    context: &DerivedContext,
    resolved: &mut Resolution,
    method_id: &str,
    parameter: &str,
    object: Object,
    quantity_key: &'static str,
) -> Result<f64, RuleRefusal> {
    let system_mass =
        context.epoch().system_weight_newtons / context.gravity_behind(Some(quantity_key));
    if object == Object::System {
        return Ok(system_mass);
    }

    let Some(body_mass) = context.body_mass_kilograms else {
        return Err(RuleRefusal::Refused(Box::new(
            Refusal::required_parameter_unstated(method_id, BODY_MASS_GLOBAL),
        )));
    };
    resolved.record_measured(
        BODY_MASS_GLOBAL,
        body_mass,
        crate::resolution::format_number(body_mass),
        plateforce_core::provenance::ParameterSource::Stated,
    );
    match object {
        Object::Body => Ok(body_mass),
        _ if body_mass <= system_mass => Ok(system_mass - body_mass),
        _ => Err(RuleRefusal::Refused(Box::new(Refusal::value_not_accepted(
            method_id,
            parameter,
            body_mass,
            vec![format!(
                "a body mass at or below the {system_mass:.4} kg this recording weighed, above \
                 which the bar it leaves is of negative mass"
            )],
        )))),
    }
}
