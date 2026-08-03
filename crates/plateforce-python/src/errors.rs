//! Python exception types, and the mapping that carries a core error across intact.
//!
//! A core failure already names the method and the parameter that failed. Flattening one
//! into a generic exception would throw away the only part a user can act on, so the
//! message is passed through verbatim and the same fields are attached to the instance.

use plateforce_core::TrialError as CoreTrialError;
use plateforce_core::{Refusal, RefusalCode};
use plateforce_registry::RegistryError as CoreRegistryError;
use pyo3::exceptions::PyException;
use pyo3::prelude::*;

pyo3::create_exception!(
    plateforce,
    PlateforceError,
    PyException,
    "Base of every error raised by this package."
);
pyo3::create_exception!(
    plateforce,
    TrialError,
    PlateforceError,
    "A trace could not be read, or a rule found nothing in it."
);
pyo3::create_exception!(
    plateforce,
    NoCrossingError,
    TrialError,
    "A detection rule found no crossing. Carries method_id, parameter, value and search_bound_seconds."
);
pyo3::create_exception!(
    plateforce,
    CollapsedBandError,
    TrialError,
    "A noise-relative band collapsed to nothing, so the rule had nothing to search. Carries method_id, parameter, value, dispersion_newtons and threshold_newtons."
);
pyo3::create_exception!(
    plateforce,
    RegistryError,
    PlateforceError,
    "The registry could not be loaded, or failed one of the rules in docs/schema.md."
);
pyo3::create_exception!(
    plateforce,
    MethodError,
    PlateforceError,
    "A method could not be selected."
);
pyo3::create_exception!(
    plateforce,
    ParameterError,
    MethodError,
    "A parameter was unknown, missing, or not a finite number. Carries method_id and parameter."
);
pyo3::create_exception!(
    plateforce,
    MethodNotImplementedError,
    MethodError,
    "The registry describes this method and no rule is available to run it."
);

/// Preserves the core message and re-attaches its fields, so a caller can branch on the
/// parameter that failed rather than parse the sentence.
pub fn map_trial_error(python: Python<'_>, error: CoreTrialError) -> PyErr {
    let message = error.to_string();
    match error {
        CoreTrialError::NoCrossing {
            method_id,
            parameter,
            value,
            search_bound_seconds,
        } => {
            let raised = NoCrossingError::new_err(message);
            let instance = raised.value(python);
            let _ = instance.setattr("method_id", method_id);
            let _ = instance.setattr("parameter", parameter);
            let _ = instance.setattr("value", value);
            let _ = instance.setattr("search_bound_seconds", search_bound_seconds);
            raised
        }
        CoreTrialError::CollapsedBand {
            method_id,
            parameter,
            value,
            dispersion_newtons,
            threshold_newtons,
        } => {
            let raised = CollapsedBandError::new_err(message);
            let instance = raised.value(python);
            let _ = instance.setattr("method_id", method_id);
            let _ = instance.setattr("parameter", parameter);
            let _ = instance.setattr("value", value);
            let _ = instance.setattr("dispersion_newtons", dispersion_newtons);
            let _ = instance.setattr("threshold_newtons", threshold_newtons);
            raised
        }
        _ => TrialError::new_err(message),
    }
}

/// Which class a published refusal code is raised as, read off the same status the engine
/// gives it rather than decided here.
///
/// The match takes no wildcard arm, so a new code has to be ruled on rather than falling
/// through to whichever class happened to be last. Two codes take a class of their own
/// because a caller catches them by name: a rule that found no crossing and a band that
/// collapsed to nothing are the two a pipeline retries differently.
fn class_of(code: RefusalCode) -> fn(String) -> PyErr {
    match code {
        RefusalCode::NoCrossing => NoCrossingError::new_err,
        RefusalCode::CollapsedBand => CollapsedBandError::new_err,
        RefusalCode::TraceTooShort
        | RefusalCode::ColumnNotFound
        | RefusalCode::TrialIdentityUnparsed
        | RefusalCode::AmbiguousForceChannels
        | RefusalCode::SchemaUnsupported
        | RefusalCode::ObservationsNotPaired
        | RefusalCode::NotEnoughObservations
        | RefusalCode::DependencyUnresolved
        | RefusalCode::FileNotRead => TrialError::new_err,
        RefusalCode::MethodNotImplemented => MethodNotImplementedError::new_err,
        RefusalCode::UnknownParameter
        | RefusalCode::ParameterNotFinite
        | RefusalCode::ValueNotAccepted
        | RefusalCode::RequiredParameterUnstated => ParameterError::new_err,
        RefusalCode::SentinelConventionUnknown
        | RefusalCode::DecisionNotMade
        | RefusalCode::PlateNotLevel
        | RefusalCode::ConventionsNotComparable => MethodError::new_err,
        RefusalCode::RegistryInvalid => RegistryError::new_err,
    }
}

/// A refusal the engine recorded, raised with every field it carries.
///
/// The fields are the ones an R condition carries, under the same names and the same code,
/// which is what makes a refusal one thing across the two languages rather than two. The
/// class is chosen from the code so a caller that catches by type and one that reads `code`
/// are reading one decision.
pub fn raise_refusal(python: Python<'_>, refusal: &Refusal) -> PyErr {
    let raised = class_of(refusal.code)(refusal.message().to_string());
    let instance = raised.value(python);
    // Everything the rule read while declining, reachable as a mapping and as an attribute
    // each. The mapping is the shape an R condition carries, so a caller crossing the two
    // languages reads one thing; the attributes are how a Python caller reaches a number
    // without a lookup. Written before the named fields, so a name the two share resolves to
    // the field rather than to a reading of the same name.
    for (name, value) in &refusal.detail {
        let _ = instance.setattr(name.as_str(), *value);
    }
    let _ = instance.setattr("detail", refusal.detail.clone());
    let _ = instance.setattr("code", refusal.code.wire_name());
    let _ = instance.setattr("method_id", refusal.method_id.clone());
    let _ = instance.setattr("slot", refusal.slot.clone());
    let _ = instance.setattr("parameter", refusal.parameter.clone());
    let _ = instance.setattr("value", refusal.value);
    let _ = instance.setattr("named_value", refusal.named_value.clone());
    let _ = instance.setattr("available", refusal.available.clone());
    raised
}

/// Registry violations arrive as one multi-line message listing every rule broken, which
/// is what the loader already produces.
pub fn map_registry_error(error: CoreRegistryError) -> PyErr {
    RegistryError::new_err(error.to_string())
}

pub fn parameter_error(method_id: &str, parameter: &str, message: String) -> PyErr {
    let raised = ParameterError::new_err(message);
    Python::attach(|python| {
        let instance = raised.value(python);
        let _ = instance.setattr("method_id", method_id);
        let _ = instance.setattr("parameter", parameter);
    });
    raised
}

pub fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    let python = module.py();
    module.add("PlateforceError", python.get_type::<PlateforceError>())?;
    module.add("TrialError", python.get_type::<TrialError>())?;
    module.add("NoCrossingError", python.get_type::<NoCrossingError>())?;
    module.add(
        "CollapsedBandError",
        python.get_type::<CollapsedBandError>(),
    )?;
    module.add("RegistryError", python.get_type::<RegistryError>())?;
    module.add("MethodError", python.get_type::<MethodError>())?;
    module.add("ParameterError", python.get_type::<ParameterError>())?;
    module.add(
        "MethodNotImplementedError",
        python.get_type::<MethodNotImplementedError>(),
    )?;
    Ok(())
}
