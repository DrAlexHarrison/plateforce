//! `takeoff.threshold.flight_noise_k_sd`: the threshold re-estimated from the flight phase
//! the seed threshold found.

use plateforce_core::provenance::ParameterSource;
use plateforce_core::takeoff::takeoff_reestimated_flight_threshold;
use plateforce_core::{Trial, WeighingEpoch};

use crate::resolution::{Resolution, RuleRefusal};

const ID: &str = "takeoff.threshold.flight_noise_k_sd";

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
    super::record_search_floor_at_weighing_epoch_end(trial, epoch, resolved);
    // `trim_fraction` trims both ends of the provisional flight phase, which is the middle
    // fraction the entry names. A caller asking for either of the other two published windows
    // is asking for a different span of the same flight, and is refused by name.
    resolved.entailed(ID, "flight_window", "middle_fraction_of_flight")?;
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
            ParameterSource::Measured,
        );
        flight.takeoff_index
    })
    .map_err(RuleRefusal::Trial)
}
