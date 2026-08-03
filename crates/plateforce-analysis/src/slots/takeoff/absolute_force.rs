//! `takeoff.threshold.absolute_force`: the first sustained run below a residual threshold.

use plateforce_core::provenance::ParameterSource;
use plateforce_core::takeoff::takeoff_first_sustained_run;
use plateforce_core::{Trial, WeighingEpoch};

use crate::resolution::{Resolution, RuleRefusal};

/// The residual threshold three of the four takeoff rules compare against.
pub(crate) const SEED_PARAMETER: &str = "threshold_n";
pub(crate) const SEED_DEFAULT_NEWTONS: f64 = 20.0;

pub(crate) fn crossing(
    trial: &Trial,
    epoch: &WeighingEpoch,
    threshold_newtons: f64,
    resolved: &mut Resolution,
) -> Result<usize, RuleRefusal> {
    let rate = trial.sample_rate_hz();
    // This rule takes the first qualifying run and the longest-run rule records under this
    // same entry, so the selection is written out rather than left to be inferred from the
    // absence of the other. A reader comparing two records reads two values, not a value
    // against a silence.
    resolved.record("selection", "first".into(), ParameterSource::Assumed);
    let minimum_flight_samples = resolved
        .milliseconds_as_samples("persistence_ms", 0.0, rate)
        .max(1);
    let comparison = resolved.residual_comparison()?;
    takeoff_first_sustained_run(
        trial.force(),
        threshold_newtons,
        minimum_flight_samples,
        comparison,
        epoch.end_index,
        rate,
    )
    .map_err(RuleRefusal::Trial)
}
