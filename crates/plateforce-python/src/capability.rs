//! What this surface can do, reported by executing this surface.
//!
//! Every surface answers the same question about itself and the answers are recorded under
//! their own names in one committed file. The operations are read off the module's own
//! exports, so an entry point that goes away shortens the array and the comparison fails. A
//! manifest generated once inside the shared crate would agree with itself whatever any
//! surface could actually do, and would have caught nothing.

use plateforce_analysis::capability::{capability, Operation, OutputFormat};
use pyo3::prelude::*;
use pyo3::types::PyModule;

/// What each name this module hands a caller can be asked to do, keyed by the name a reader
/// types.
///
/// None for a name with no ruling, which the test in `tests/test_capability.py` refuses, so
/// an entry point added without a decision about the manifest cannot pass as one that
/// dispatches nothing. An empty slice is that decision, made: a class that carries a value
/// outward performs none of the operations this manifest asserts over.
fn operations_named(entry_point: &str) -> Option<&'static [Operation]> {
    match entry_point {
        "analyse_countermovement_jump" => Some(&[Operation::Analyse]),
        "read_force_file" => Some(&[Operation::ParseForceFile]),
        "spread" => Some(&[Operation::Spread]),
        "capability_json" => Some(&[Operation::Capability]),
        // A run over a folder reads every trial in it, so it parses force files as well as
        // looping the analysis. It reaches `plateforce_batch::analyse` and not `compare`, so
        // `batch` appears here and `compare` does not.
        "batch" => Some(&[Operation::Batch, Operation::ParseForceFile]),
        // One class answers all three: it reports the census, hands back every entry, and
        // refuses to exist at all for a registry that fails its own validator, which is how
        // this surface validates.
        "Registry" => Some(&[
            Operation::RegistryCensus,
            Operation::RegistryShow,
            Operation::RegistryValidate,
        ]),
        "__version__" => Some(&[Operation::Version]),

        // Values and records that travel outward. None of them is a computation this
        // manifest asserts over.
        "Acquisition" | "Bias" | "BatchResult" | "BatchRun" | "BoundMethod" | "Census"
        | "Citation" | "Construct" | "CountermovementJump" | "Disagreement" | "Exclusions"
        | "Failure" | "Gui" | "Measured" | "MethodEntry" | "Parameter" | "Preset"
        | "Provenance" | "ProvenanceStep" | "ReadReport" | "Sentinel" | "SentinelPartition"
        | "Spread" | "SpreadVariant" | "Trial" | "TrialIdentity" => Some(&[]),

        // The exception hierarchy a caller catches by type.
        "CollapsedBandError" | "MethodError" | "MethodNotImplementedError" | "NoCrossingError"
        | "ParameterError" | "PlateforceError" | "RegistryError" | "TrialError" => Some(&[]),

        // Rules the research harness reaches directly rather than through an analysis, and
        // one identity computed from a number a plate did not measure. Each is a call into
        // the core, none is one of the named operations.
        "classify_low_force_runs" | "jump_height_from_flight_time" | "partition_sentinel_values"
        | "rise_after_run" | "rise_looks_like_a_landing" | "takeoff_by_landing_shape" => {
            Some(&[])
        }
        _ => None,
    }
}

/// Every name this module hands a caller.
///
/// Read off the module rather than from a list beside it, which is what makes this an answer
/// about the wheel that is running rather than about the source somebody last edited.
///
/// Two exclusions, both because the name is not something a caller can ask to do anything.
/// A name beginning with an underscore is a helper this package reaches for itself, and
/// `__version__` is put back because it is an answer. A module is a namespace rather than an
/// entry point: an installed wheel is a package holding the compiled extension under the
/// package's own name, so without this the surface reports itself as a thing it offers.
pub fn entry_points(python: Python<'_>) -> PyResult<Vec<String>> {
    let module = PyModule::import(python, "plateforce")?;
    let mut named = vec!["__version__".to_string()];
    for name in module.dir()?.iter() {
        let name = name.to_string();
        if name.starts_with('_') {
            continue;
        }
        if module.getattr(name.as_str())?.is_instance_of::<PyModule>() {
            continue;
        }
        named.push(name);
    }
    named.sort();
    Ok(named)
}

/// The operations this wheel reaches, and the names it offers that nothing has ruled on.
pub fn operations_and_unmapped(python: Python<'_>) -> PyResult<(Vec<Operation>, Vec<String>)> {
    let mut operations = Vec::new();
    let mut unmapped = Vec::new();
    for name in entry_points(python)? {
        match operations_named(&name) {
            Some(reached) => operations.extend_from_slice(reached),
            None => unmapped.push(name),
        }
    }
    Ok((operations, unmapped))
}

/// What this surface writes a result into, taken from what it writes.
///
/// A batch run's two writers, and nothing else: an analysis hands back objects rather than
/// writing a container. Parquet is compiled in behind a feature, so a build without it
/// reports one writer and says so rather than claiming a container the wheel cannot produce.
fn every_output_format() -> Vec<OutputFormat> {
    let mut written = vec![OutputFormat::Csv, OutputFormat::Json];
    #[cfg(feature = "parquet")]
    written.push(OutputFormat::Parquet);
    written
}

/// What this surface can be asked to do, in the envelope every surface answers in.
///
/// The bytes are the shape a comparison reads: sorted keys and no spacing, so a difference
/// against another surface is a plain diff rather than a question about which map type a
/// build selected.
#[pyfunction]
pub fn capability_json(python: Python<'_>) -> PyResult<String> {
    let (operations, _) = operations_and_unmapped(python)?;
    let manifest = capability(&operations, &every_output_format());
    let value = serde_json::to_value(manifest)
        .map_err(|error| crate::errors::TrialError::new_err(error.to_string()))?;
    serde_json::to_string(&sorted(&serde_json::json!({ "ok": value })))
        .map_err(|error| crate::errors::TrialError::new_err(error.to_string()))
}

/// `serde_json::Map` preserves insertion order unless the `preserve_order` feature is off,
/// in which case it is already a `BTreeMap`. Sorting here makes the output independent of
/// which of the two a build selected.
fn sorted(value: &serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(fields) => serde_json::Value::Object(
            fields
                .iter()
                .map(|(key, held)| (key.clone(), sorted(held)))
                .collect::<serde_json::Map<String, serde_json::Value>>()
                .into_iter()
                .collect::<std::collections::BTreeMap<_, _>>()
                .into_iter()
                .collect(),
        ),
        serde_json::Value::Array(items) => {
            serde_json::Value::Array(items.iter().map(sorted).collect())
        }
        held => held.clone(),
    }
}

/// The names this wheel offers that no ruling covers, for the test that refuses them.
#[pyfunction]
#[pyo3(name = "_entry_points_with_no_operations_ruled")]
pub fn entry_points_with_no_operations_ruled(python: Python<'_>) -> PyResult<Vec<String>> {
    Ok(operations_and_unmapped(python)?.1)
}
