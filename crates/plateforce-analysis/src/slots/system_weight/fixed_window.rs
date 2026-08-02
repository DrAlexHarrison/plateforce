//! `bwepoch.fixed_window`: a window of stated length at a stated start.

use plateforce_core::provenance::ParameterSource;
use plateforce_core::trial::CentralTendency;
use plateforce_core::{DispersionEstimator, Trial, WeighingEpoch};

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
) -> Result<WeighingEpoch, String> {
    let centre = resolved
        .enumerated(
            "centre",
            "mean",
            &[
                ("mean", CentralTendency::Mean),
                ("median", CentralTendency::Median),
            ],
        )
        .map_err(|refused| refused.to_string())?;
    let epoch = weighing_epoch_at(
        trial,
        choice.start_index.unwrap_or(0),
        duration_seconds,
        centre,
        dispersion,
    )?;
    resolved.record_measured(
        "start_seconds",
        trial.time_at(epoch.start_index),
        format!("{:.4}", trial.time_at(epoch.start_index)),
        if choice.start_index.is_some() {
            ParameterSource::Stated
        } else {
            ParameterSource::Measured
        },
    );
    Ok(epoch)
}
