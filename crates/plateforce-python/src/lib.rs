//! Python surface over `plateforce-core` and `plateforce-registry`.
//!
//! The math is not here and must never be. Every number this module returns came out of
//! the one implementation in the core, and what this layer adds is the record of which
//! method produced it and what that method was bound to.
//!
//! `Acquisition` and `Sentinel` are the only classes a caller passes back in; the rest
//! travel outward only, and every class states which of the two it is.

mod analysis;
mod errors;
mod registry;
mod result;
mod trial;

use pyo3::prelude::*;

#[pymodule]
fn plateforce(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add("__version__", env!("CARGO_PKG_VERSION"))?;

    module.add_class::<trial::Trial>()?;
    module.add_class::<trial::Acquisition>()?;
    module.add_class::<trial::Sentinel>()?;
    module.add_class::<trial::SentinelPartition>()?;

    module.add_class::<registry::Registry>()?;
    module.add_class::<registry::MethodEntry>()?;
    module.add_class::<registry::BoundMethod>()?;
    module.add_class::<registry::Parameter>()?;
    module.add_class::<registry::Citation>()?;
    module.add_class::<registry::Bias>()?;
    module.add_class::<registry::Failure>()?;
    module.add_class::<registry::Disagreement>()?;
    module.add_class::<registry::Gui>()?;
    module.add_class::<registry::Construct>()?;
    module.add_class::<registry::Census>()?;

    module.add_class::<result::Measured>()?;
    module.add_class::<result::Provenance>()?;
    module.add_class::<result::Exclusions>()?;
    module.add_class::<analysis::CountermovementJump>()?;

    module.add_function(wrap_pyfunction!(
        analysis::analyse_countermovement_jump,
        module
    )?)?;
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

    errors::register(module)
}
