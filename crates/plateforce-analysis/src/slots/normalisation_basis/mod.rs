//! What a force, rate, power, work or impulse is expressed relative to.
//!
//! Relative power in the weightlifting literature and in strength and conditioning are
//! different quantities with the same name: one normalises to barbell mass and the other to
//! body mass, and across a competition field the athlete-to-bar ratio varies by a factor of
//! two. A per-kilogram label that does not say which kilograms breaks cross-study comparison
//! without ever looking wrong.

pub mod denominator;

/// The construct id, as `registry/constructs.toml` declares it.
pub const CONSTRUCT: &str = "normalisation_basis";

/// The key the declaration reports the divisor under.
pub const KEY: &str = "normalisation_denominator_kilograms";

pub mod absolute;
pub mod allometric;
pub mod dimensionless_hof;
pub mod percent_of_peak_force;
pub mod ratio_body_mass;

use crate::derived::DerivedContext;
use crate::resolution::RuleRefusal;

/// The athlete's mass, and the record that a number divided by it rests on the caller having
/// stated it.
///
/// The weighed system mass is not it and a rule here declines rather than dividing by the
/// other one: on a loaded lift the two differ by the bar, and a per-kilogram number that
/// silently used the wrong kilograms is the failure this construct exists to name.
pub(crate) fn body_mass_kilograms(
    context: &DerivedContext,
    resolved: &mut crate::resolution::Resolution,
    method_id: &str,
) -> Result<f64, RuleRefusal> {
    let Some(kilograms) = context.body_mass_kilograms else {
        return Err(RuleRefusal::Refused(Box::new(
            plateforce_core::Refusal::required_parameter_unstated(
                method_id,
                crate::request::BODY_MASS_GLOBAL,
            ),
        )));
    };
    resolved.record_measured(
        crate::request::BODY_MASS_GLOBAL,
        kilograms,
        crate::resolution::format_number(kilograms),
        plateforce_core::provenance::ParameterSource::Stated,
    );
    Ok(kilograms)
}

/// A quantity another construct's chosen rule reported, and the entry that reported it.
///
/// A normalisation rule divides a result rather than the force trace, so the number it starts
/// from is whichever rule the caller picked for that construct. Taking a second reading off
/// the trace here would divide a peak this analysis never reported, under a convention nobody
/// chose, and the two would agree until they did not.
pub(crate) fn measured(
    context: &DerivedContext,
    method_id: &str,
    construct: &'static str,
    key: &str,
) -> Result<(f64, Option<String>), RuleRefusal> {
    match context.measured(key) {
        Some(found) => Ok((found.value, found.computed_by.clone())),
        None => Err(context.unavailable(method_id, &[construct])),
    }
}

/// A stated mass no scaling can be built on, named back to the caller with the value they
/// stated. Zero and below are the whole of it: dividing by either reports a sign flip or an
/// infinity as a measurement.
pub(crate) fn mass_not_accepted(method_id: &str, kilograms: f64) -> RuleRefusal {
    RuleRefusal::Refused(Box::new(plateforce_core::Refusal::value_not_accepted(
        method_id,
        crate::request::BODY_MASS_GLOBAL,
        kilograms,
        vec!["a body mass above zero".to_string()],
    )))
}
