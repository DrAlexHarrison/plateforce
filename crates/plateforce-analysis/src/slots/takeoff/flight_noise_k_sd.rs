//! `takeoff.threshold.flight_noise_k_sd`: the threshold re-estimated from the flight phase
//! the seed threshold found.

use plateforce_core::takeoff::takeoff_reestimated_flight_threshold;
use plateforce_core::{Trial, WeighingEpoch};

use crate::resolution::{Resolution, RuleRefusal};

/// This rule's seed is the bounding threshold, which the registry files separately from the
/// residual threshold the other three rules compare against.
pub(crate) const SEED_PARAMETER: &str = "bounding_threshold_n";
pub(crate) const SEED_DEFAULT_NEWTONS: f64 = 10.0;

pub(crate) fn crossing(
    trial: &Trial,
    epoch: &WeighingEpoch,
    threshold_newtons: &mut f64,
    resolved: &mut Resolution,
) -> Result<usize, RuleRefusal> {
    let rate = trial.sample_rate_hz();
    let trim_fraction = resolved.number("trim_fraction", 0.25);
    let k = resolved.number("k", 5.0);
    let dispersion = resolved.dispersion()?;
    takeoff_reestimated_flight_threshold(
        trial.force(),
        epoch.end_index,
        *threshold_newtons,
        trim_fraction,
        k,
        dispersion,
        rate,
    )
    .map(|flight| {
        *threshold_newtons = flight.threshold_newtons;
        // The seed threshold above is replaced by one measured from the flight
        // phase, and it is that one every later comparison runs against.
        resolved.record_measured(
            "reestimated_threshold_newtons",
            flight.threshold_newtons,
            format!("{:.4}", flight.threshold_newtons),
            false,
        );
        flight.takeoff_index
    })
    .map_err(RuleRefusal::Trial)
}
