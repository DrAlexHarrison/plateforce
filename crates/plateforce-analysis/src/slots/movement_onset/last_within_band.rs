//! `onset.threshold.last_within_band`: the last sample still inside the noise band,
//! reached by working backwards from the force minimum.

use plateforce_core::onset::{onset_last_sample_within_noise_band, PostCrossingRule};
use plateforce_core::provenance::ParameterSource;
use plateforce_core::statistics::index_of_maximum;
use plateforce_core::takeoff::force_minimum_index;
use plateforce_core::{Trial, WeighingEpoch};

use crate::resolution::{Resolution, RuleRefusal};
use crate::slots::movement_onset::record_inherited_spread;

/// This rule resolves its own backtrack, through `PostCrossingRule`.
pub(crate) const APPLIES_BACKTRACK: bool = false;

/// The countermovement dip, which is the force minimum reached before the propulsive peak.
///
/// Scoped to the whole recording the minimum is a flight sample, and searching back from
/// there stops at the propulsion phase: on the six committed trials that placed onset 310
/// to 396 ms late and returned jump heights of 0.73 to 0.82 m against 0.41 to 0.44 m from
/// the other four rules. Bounded here the two crossings the registry says a
/// countermovement trace makes become one, and this rule and `onset.threshold.noise_relative`
/// return the same height on all six.
fn countermovement_dip(force: &[f64], takeoff_index: usize) -> Option<usize> {
    let propulsive_peak = index_of_maximum(&force[..takeoff_index.min(force.len())])?;
    force_minimum_index(force, propulsive_peak)
}

pub(crate) fn crossing(
    trial: &Trial,
    epoch: &WeighingEpoch,
    takeoff_index: Option<usize>,
    inherited_spread: (&str, bool),
    resolved: &mut Resolution,
    warnings: &mut Vec<String>,
) -> Result<usize, RuleRefusal> {
    let force = trial.force();
    let rate = trial.sample_rate_hz();
    let k = resolved.number("k", 5.0);
    record_inherited_spread(resolved, inherited_spread);
    let lookback_samples = resolved.seconds_as_samples("inverse_lookback", 0.5, rate);
    let back_offset_samples = resolved.milliseconds_as_samples("offset_ms", 30.0, rate);

    let search_end = takeoff_index
        .and_then(|takeoff| countermovement_dip(force, takeoff))
        .ok_or_else(|| {
            RuleRefusal::Stated(
                "onset.threshold.last_within_band searches back from the countermovement dip, \
                 which is the force minimum before the propulsive peak, and this recording \
                 settles no takeoff to bound that peak"
                    .to_string(),
            )
        })?;
    resolved.record_measured(
        "search_bound_seconds",
        trial.time_at(search_end),
        format!("{:.4}", trial.time_at(search_end)),
        ParameterSource::Measured,
    );

    onset_last_sample_within_noise_band(
        force,
        epoch.system_weight_newtons,
        epoch.standard_deviation_newtons,
        k,
        search_end,
        lookback_samples,
        PostCrossingRule::FixedOffset(back_offset_samples),
        rate,
    )
    .map(|outcome| {
        if outcome.clamped_at_start {
            warnings.push(
                "the onset backtrack ran off the front of the recording and was clamped at sample zero".into(),
            );
        }
        outcome.index
    })
    .map_err(RuleRefusal::Trial)
}
