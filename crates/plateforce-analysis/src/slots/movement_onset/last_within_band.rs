//! `onset.threshold.last_within_band`: the last sample still inside the noise band,
//! reached by working backwards from the force minimum.

use plateforce_core::onset::{onset_last_sample_within_noise_band, PostCrossingRule};
use plateforce_core::{Trial, WeighingEpoch};

use crate::resolution::{Resolution, RuleRefusal};
use crate::slots::movement_onset::record_inherited_spread;

/// This rule resolves its own backtrack, through `PostCrossingRule`.
pub(crate) const APPLIES_BACKTRACK: bool = false;

pub(crate) fn crossing(
    trial: &Trial,
    epoch: &WeighingEpoch,
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
    onset_last_sample_within_noise_band(
        force,
        epoch.system_weight_newtons,
        epoch.standard_deviation_newtons,
        k,
        plateforce_core::takeoff::force_minimum_index(force, trial.len())
            .unwrap_or(trial.len() - 1),
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
