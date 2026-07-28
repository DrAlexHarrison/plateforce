//! Python exception types, and the mapping that carries a core error across intact.
//!
//! A core failure already names the method and the parameter that failed. Flattening one
//! into a generic exception would throw away the only part a user can act on, so the
//! message is passed through verbatim and the same fields are attached to the instance.

use plateforce_core::TrialError as CoreTrialError;
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
