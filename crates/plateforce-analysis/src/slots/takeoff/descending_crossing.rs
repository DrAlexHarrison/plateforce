//! `takeoff.threshold.descending_crossing`: the sample before a crossing confirmed to hold.

use plateforce_core::takeoff::takeoff_descending_crossing;
use plateforce_core::Trial;

use crate::resolution::{Resolution, RuleRefusal};

pub(crate) fn crossing(
    trial: &Trial,
    threshold_newtons: f64,
    resolved: &mut Resolution,
) -> Result<usize, RuleRefusal> {
    let rate = trial.sample_rate_hz();
    // A crossing this rule calls confirmed has to have a span to be confirmed over,
    // so unstated it takes the shortest span the persistence operator publishes.
    let confirmation_samples = resolved
        .milliseconds_as_samples("persistence_ms", 20.0, rate)
        .max(1);
    takeoff_descending_crossing(trial.force(), threshold_newtons, confirmation_samples, rate)
        .map_err(RuleRefusal::Trial)
}
