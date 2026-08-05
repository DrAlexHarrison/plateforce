//! `bwepoch.adaptive_lowest_variance`: the quietest window the recording holds.

use plateforce_core::provenance::ParameterSource;
use plateforce_core::statistics::{median, WeighingWindowSearch};
use plateforce_core::{DispersionEstimator, Refusal, Trial, VarianceAccumulation, WeighingEpoch};

use crate::resolution::Resolution;

pub(crate) const WINDOW_LENGTH_PARAMETER: &str = "window_seconds";

/// Fallback for the gate below, matching the entry's declared default. A fixed 20 N gate is
/// 2.5 percent of an 80 kg athlete and 1.7 percent of a 120 kg one, so the same nominal rule
/// tests the two differently.
const REJECT_AT_OR_BELOW_FRACTION_OF_WEIGHT: f64 = 0.025;

/// biomex clamps the searched spread up to a fraction of bodyweight before any threshold is
/// scaled by it. On that tool's own demonstration trial the clamp binds, 0.922 N measured
/// against a 4.071 N floor, and the rule then collapses to a fixed 2.5 percent band.
fn apply_variance_floor(
    epoch: &mut WeighingEpoch,
    resolved: &mut Resolution,
    warnings: &mut Vec<String>,
) {
    let floor_percent_of_weight = resolved.number("variance_floor_pct_bodyweight", 0.0);
    let floor_newtons = epoch.system_weight_newtons * floor_percent_of_weight / 100.0;
    if floor_newtons > epoch.standard_deviation_newtons {
        warnings.push(format!(
            "the weighing window's spread is {:.3} N and the floor is {:.3} N, so every noise-relative threshold below is set by the floor and not by this recording",
            epoch.standard_deviation_newtons, floor_newtons
        ));
        epoch.standard_deviation_newtons = floor_newtons;
    }
}

/// How many windows the rule compared and how many the low-force gate removed before it
/// compared anything.
///
/// The two travel together: the rejected count on its own says nothing about how much of the
/// recording was ruled out, and the candidate count on its own hides that anything was.
fn record_window_counts(search: &WeighingWindowSearch, resolved: &mut Resolution) {
    for (name, count) in [
        (COMPARED_WINDOW_COUNT, search.candidate_window_count),
        (REJECTED_WINDOW_COUNT, search.rejected_window_count),
    ] {
        resolved.record_measured(
            name,
            count as f64,
            count.to_string(),
            ParameterSource::Measured,
        );
    }
}

/// The names the two counts are recorded under, read from here by the reader that looks them
/// up again.
pub const COMPARED_WINDOW_COUNT: &str = "compared_window_count";
pub const REJECTED_WINDOW_COUNT: &str = "rejected_window_count";

pub(crate) fn search(
    trial: &Trial,
    duration_seconds: f64,
    dispersion: DispersionEstimator,
    resolved: &mut Resolution,
    warnings: &mut Vec<String>,
) -> Result<WeighingEpoch, Box<Refusal>> {
    let window_samples = (duration_seconds * trial.sample_rate_hz()).round() as usize;
    let accumulation = resolved
        .enumerated(
            "accumulation",
            &[
                (
                    "cumulative_sum_of_squares",
                    VarianceAccumulation::CumulativeSumOfSquares,
                ),
                ("two_pass", VarianceAccumulation::TwoPass),
            ],
        )
        .map_err(Refusal::from)?;
    // The unloaded plate is the quietest window in any recording, so the gate is taken
    // against the weight the trace carries for most of its length.
    let reject_at_or_below_fraction_of_weight = resolved.number(
        "reject_at_or_below_fraction_of_weight",
        REJECT_AT_OR_BELOW_FRACTION_OF_WEIGHT,
    );
    let provisional_weight_newtons = median(trial.force()).unwrap_or_default();
    let reject_at_or_below_newtons =
        provisional_weight_newtons * reject_at_or_below_fraction_of_weight;
    resolved.record_measured(
        "reject_at_or_below_newtons",
        reject_at_or_below_newtons,
        format!("{reject_at_or_below_newtons:.4}"),
        ParameterSource::Measured,
    );
    let (mut epoch, search) = WeighingEpoch::lowest_variance(
        trial,
        window_samples,
        trial.len(),
        Some(reject_at_or_below_newtons),
        accumulation,
        dispersion,
    )
    .map_err(Refusal::from)?;
    // The gate above takes windows out of the running, 985 of 4801 on subject 01's first
    // trial, and a rule that removed a fifth of what it compared has to say so. Both counts,
    // because a rejection count without the population it came from is not a proportion.
    record_window_counts(&search, resolved);
    // A minimum with exact ties does not identify one window, and on this corpus the
    // tie has run to 138 windows on the worst trial.
    if epoch.tied_window_count > 1 {
        warnings.push(format!(
            "the lowest-variance rule found {} windows tied on variance, so it does not identify a single weighing epoch",
            epoch.tied_window_count
        ));
    }
    // This rule finds the window in the trace rather than being told where it sits.
    resolved.record_measured(
        "start_seconds",
        trial.time_at(epoch.start_index),
        format!("{:.4}", trial.time_at(epoch.start_index)),
        ParameterSource::Measured,
    );
    apply_variance_floor(&mut epoch, resolved, warnings);
    Ok(epoch)
}
