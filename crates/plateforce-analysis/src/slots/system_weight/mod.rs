//! The weighing epoch, and with it system weight, the onset band and every impulse below.

pub mod adaptive_lowest_variance;
pub mod fixed_window;
pub mod manual_placement;

use plateforce_core::trial::CentralTendency;
use plateforce_core::{DispersionEstimator, Trial, WeighingEpoch};

use crate::request::WeighingChoice;
use crate::resolution::{dispersion_label, BoundValues, Resolution};

pub(crate) struct WeighingOutcome {
    pub epoch: WeighingEpoch,
    pub bound: BoundValues,
    /// The convention this window's spread was computed under. Every noise-relative
    /// threshold below is scaled by it, and the registry files it on the onset row.
    pub standard_deviation_convention: &'static str,
    pub standard_deviation_convention_stated: bool,
}

/// The registry names the window's length on each weighing rule's own row, and the three
/// rows do not agree on the name.
pub(crate) fn window_length_parameter(method_id: &str) -> &'static str {
    match method_id {
        "bwepoch.adaptive_lowest_variance" => adaptive_lowest_variance::WINDOW_LENGTH_PARAMETER,
        "bwepoch.manual_placement" => manual_placement::WINDOW_LENGTH_PARAMETER,
        _ => fixed_window::WINDOW_LENGTH_PARAMETER,
    }
}

/// A weighing window at an arbitrary start, without a second implementation of the mean
/// and the standard deviation. `WeighingEpoch::fixed_window` anchors at sample zero, so
/// the window is fed a trace that starts where the window does and the indices are
/// restated against the original trace afterwards.
pub fn weighing_epoch_at(
    trial: &Trial,
    start_index: usize,
    duration_seconds: f64,
    centre: CentralTendency,
    dispersion: DispersionEstimator,
) -> Result<WeighingEpoch, String> {
    if start_index == 0 {
        return WeighingEpoch::fixed_window(trial, duration_seconds, centre, dispersion)
            .map_err(|e| e.to_string());
    }
    let start_index = start_index.min(trial.len().saturating_sub(2));
    let shifted = Trial::new(
        trial.force()[start_index..].to_vec(),
        trial.sample_rate_hz(),
    )
    .map_err(|e| e.to_string())?;
    let mut epoch = WeighingEpoch::fixed_window(&shifted, duration_seconds, centre, dispersion)
        .map_err(|e| e.to_string())?;
    epoch.start_index += start_index;
    epoch.end_index += start_index;
    Ok(epoch)
}

pub(crate) fn resolve(
    trial: &Trial,
    choice: &WeighingChoice,
    warnings: &mut Vec<String>,
) -> Result<WeighingOutcome, String> {
    let mut resolved = Resolution::over(&choice.parameters, &choice.options);
    let duration_seconds = resolved.number(window_length_parameter(&choice.method_id), 1.0);
    let standard_deviation_convention_stated = choice.options.contains_key("dispersion");
    let dispersion = resolved
        .dispersion()
        .map_err(|refused| refused.to_string())?;
    let standard_deviation_convention = dispersion_label(dispersion);

    // A window placed by hand is a placed window whichever rule named it, so the searching
    // rule runs its search only when nobody has said where the window goes.
    let epoch =
        if choice.method_id == "bwepoch.adaptive_lowest_variance" && choice.start_index.is_none() {
            adaptive_lowest_variance::search(
                trial,
                duration_seconds,
                dispersion,
                &mut resolved,
                warnings,
            )?
        } else {
            fixed_window::place(trial, choice, duration_seconds, dispersion, &mut resolved)?
        };

    Ok(WeighingOutcome {
        epoch,
        bound: resolved.finish(),
        standard_deviation_convention,
        standard_deviation_convention_stated,
    })
}
