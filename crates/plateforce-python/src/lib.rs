//! Python surface over `plateforce-core` and `plateforce-registry`.
//!
//! The math is not here. Every number this module returns came out of the one
//! implementation in the core, and this layer adds the record of which method produced it
//! and what that method was bound to.
//!
//! `Acquisition` and `Sentinel` are the only classes a caller passes back in; the rest
//! travel outward only, and every class states which of the two it is.

mod analysis;
mod batch;
mod capability;
mod errors;
mod quality;
mod registry;
mod result;
mod spread;
mod trial;

use pyo3::prelude::*;

#[pymodule]
fn plateforce(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add("__version__", env!("CARGO_PKG_VERSION"))?;

    module.add_class::<trial::Trial>()?;
    module.add_class::<trial::Acquisition>()?;
    module.add_class::<trial::Sentinel>()?;
    module.add_class::<trial::SentinelPartition>()?;
    module.add_class::<trial::ReadReport>()?;

    module.add_class::<registry::Registry>()?;
    module.add_class::<registry::MethodEntry>()?;
    module.add_class::<registry::BoundMethod>()?;
    module.add_class::<registry::Preset>()?;
    module.add_class::<registry::Parameter>()?;
    module.add_class::<registry::Citation>()?;
    module.add_class::<registry::Bias>()?;
    module.add_class::<registry::Failure>()?;
    module.add_class::<registry::Disagreement>()?;
    module.add_class::<registry::Gui>()?;
    module.add_class::<registry::Construct>()?;
    module.add_class::<registry::Census>()?;

    module.add_class::<analysis::BoundGlobal>()?;
    module.add_class::<result::Measured>()?;
    module.add_class::<result::Provenance>()?;
    module.add_class::<result::Exclusions>()?;
    module.add_class::<analysis::CountermovementJump>()?;
    module.add_class::<quality::QualitySignal>()?;

    module.add_class::<spread::Spread>()?;
    module.add_class::<spread::SpreadVariant>()?;

    module.add_class::<batch::BatchResult>()?;
    module.add_class::<batch::BatchRun>()?;
    module.add_class::<batch::TrialIdentity>()?;

    module.add_function(wrap_pyfunction!(
        analysis::analyse_countermovement_jump,
        module
    )?)?;
    // The engine's own document, which the shaped answer above is built from and does not
    // keep. Private, because a caller reads the classes; the parity gate reads this.
    module.add_function(wrap_pyfunction!(analysis::analyse_json, module)?)?;
    module.add_function(wrap_pyfunction!(
        analysis::jump_height_from_flight_time,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        analysis::takeoff_by_landing_shape,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(analysis::classify_low_force_runs, module)?)?;
    module.add_function(wrap_pyfunction!(analysis::rise_after_run, module)?)?;
    module.add_function(wrap_pyfunction!(
        analysis::rise_looks_like_a_landing,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(trial::partition_sentinel_values, module)?)?;
    module.add_function(wrap_pyfunction!(trial::read_force_file, module)?)?;

    module.add_function(wrap_pyfunction!(spread::spread_over, module)?)?;
    // The engine's own record of a sweep, for the reason `_analyse_json` is here: the shaped
    // answer above is what a caller reads, and this is what a comparison against another
    // surface's sweep can be held to.
    module.add_function(wrap_pyfunction!(spread::spread_json, module)?)?;

    module.add_function(wrap_pyfunction!(capability::capability_json, module)?)?;
    module.add_function(wrap_pyfunction!(
        capability::entry_points_with_no_operations_ruled,
        module
    )?)?;

    module.add_function(wrap_pyfunction!(batch::batch, module)?)?;
    // `BatchResult.__reduce__` reaches this by name on the module, so a result cannot be
    // pickled without it, and a pool over a directory cannot hand its results back.
    module.add_function(wrap_pyfunction!(batch::batch_result_from_json, module)?)?;

    errors::register(module)
}
