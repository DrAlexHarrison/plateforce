//! `bwepoch.fixed_window`: a window of stated length at a stated start.

use plateforce_core::provenance::ParameterSource;
use plateforce_core::trial::CentralTendency;
use plateforce_core::{DispersionEstimator, Refusal, Trial, WeighingEpoch};

use crate::request::WeighingChoice;
use crate::resolution::Resolution;
use crate::slots::system_weight::weighing_epoch_at;

pub(crate) const WINDOW_LENGTH_PARAMETER: &str = "duration";

pub(crate) fn place(
    trial: &Trial,
    choice: &WeighingChoice,
    duration_seconds: f64,
    dispersion: DispersionEstimator,
    resolved: &mut Resolution,
) -> Result<WeighingEpoch, Refusal> {
    let centre = resolved
        .enumerated(
            "centre",
            "mean",
            &[
                ("mean", CentralTendency::Mean),
                ("median", CentralTendency::Median),
            ],
        )
        .map_err(Refusal::from)?;
    let epoch = weighing_epoch_at(
        trial,
        choice.start_index.unwrap_or(0),
        duration_seconds,
        centre,
        dispersion,
    )?;
    // Where the window is anchored and what time that lands on are two facts, and one
    // recorded value carried both. The caller states a sample index; the seconds it means
    // depend on the recording's rate, so a stated index reads as a different time on two
    // recordings and only the anchor is the caller's.
    let (anchor, anchor_source) = match choice.start_index {
        Some(_) => ("stated_index", ParameterSource::Stated),
        None => ("trial_start", ParameterSource::Assumed),
    };
    resolved.record("window_anchor", anchor.to_string(), anchor_source);
    resolved.record_measured(
        "start_seconds",
        trial.time_at(epoch.start_index),
        format!("{:.4}", trial.time_at(epoch.start_index)),
        ParameterSource::Measured,
    );
    Ok(epoch)
}
