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
) -> Result<WeighingEpoch, Box<Refusal>> {
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
    // Where the window is anchored and what time that lands on are two facts. The caller
    // states a sample index; the seconds it means depend on the recording's rate, so only
    // the anchor is the caller's.
    //
    // Placing the start is what settles the anchor, so an anchor written beside a start that
    // says otherwise is refused rather than dropped. `pre_signal` weighs a different span of
    // the same recording and no start index reaches it, so it is refused by the same line.
    let (anchor, anchor_source) = match choice.start_index {
        Some(_) => ("stated_index", ParameterSource::Stated),
        None => ("trial_start", ParameterSource::Assumed),
    };
    resolved
        .entailed_from(&choice.method_id, "window_anchor", anchor, anchor_source)
        .map_err(Refusal::from)?;
    resolved.record_measured(
        "start_seconds",
        trial.time_at(epoch.start_index),
        format!("{:.4}", trial.time_at(epoch.start_index)),
        ParameterSource::Measured,
    );
    Ok(epoch)
}
