//! Movement onset. Nine rules in the registry answer this question and they answer it
//! differently, so nothing here decides which one a caller wants.
//!
//! Every threshold, every persistence requirement and every backtrack arrives as a
//! parameter. A rule identified only by its multiplier of the noise standard deviation
//! does not identify a method: the same lab published the accompanying backtrack as
//! 10 ms, then 30 ms, then 50 and 40 ms.

use crate::method_ids;
use crate::signal::TrialError;
use crate::statistics::{
    index_of_maximum, index_of_minimum, mean_and_standard_deviation, DispersionEstimator,
};

/// The band a signal must leave for a crossing to count.
///
/// Absent bounds are open, so a one sided rule and a two sided rule are the same
/// function with a different band.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ExcursionBand {
    pub lower_newtons: Option<f64>,
    pub upper_newtons: Option<f64>,
}

impl ExcursionBand {
    pub fn below(threshold_newtons: f64) -> Self {
        Self {
            lower_newtons: Some(threshold_newtons),
            upper_newtons: None,
        }
    }

    pub fn above(threshold_newtons: f64) -> Self {
        Self {
            lower_newtons: None,
            upper_newtons: Some(threshold_newtons),
        }
    }

    /// Centred band, as a noise relative rule states it.
    pub fn centred(centre_newtons: f64, half_width_newtons: f64) -> Self {
        Self {
            lower_newtons: Some(centre_newtons - half_width_newtons),
            upper_newtons: Some(centre_newtons + half_width_newtons),
        }
    }

    pub fn is_outside(&self, value_newtons: f64) -> bool {
        self.lower_newtons
            .is_some_and(|lower| value_newtons < lower)
            || self
                .upper_newtons
                .is_some_and(|upper| value_newtons > upper)
    }
}

/// Which crossing of a repeatedly crossed threshold a rule takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrossingSelection {
    First,
    Last,
}

/// Where a rule is allowed to look, and how long the excursion must hold.
///
/// `start_index` is the reason two rules with the same threshold disagree by seconds:
/// several tools refuse to place onset before the end of their own quiet window, so
/// onset is bounded below by a parameter that has nothing to do with the athlete.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CrossingSearch {
    pub start_index: usize,
    pub end_index: usize,
    pub persistence_samples: usize,
    pub selection: CrossingSelection,
}

/// First or last index in the search range whose excursion holds for the required run.
pub fn sustained_excursion(
    signal: &[f64],
    band: &ExcursionBand,
    search: &CrossingSearch,
) -> Option<usize> {
    let persistence = search.persistence_samples.max(1);
    let last_startable = signal.len().checked_sub(persistence)?;
    let end = search.end_index.min(last_startable + 1);
    let mut found = None;
    // Indexing is the operation: each candidate start is tested against the window that
    // follows it, so there is no single element to iterate over.
    #[allow(clippy::needless_range_loop)]
    for index in search.start_index..end {
        if !signal[index..index + persistence]
            .iter()
            .all(|&value| band.is_outside(value))
        {
            continue;
        }
        match search.selection {
            CrossingSelection::First => return Some(index),
            CrossingSelection::Last => found = Some(index),
        }
    }
    found
}

/// Last index in the search range whose value is inside the band, which is the
/// mirror image rule: not where movement started but where quiet standing stopped.
pub fn last_sample_inside_band(
    signal: &[f64],
    band: &ExcursionBand,
    start_index: usize,
    end_index: usize,
) -> Option<usize> {
    let end = end_index.min(signal.len());
    (start_index..end)
        .rev()
        .find(|&index| !band.is_outside(signal[index]))
}

fn no_crossing(method_id: &str, parameter: &str, value: f64, bound_seconds: f64) -> TrialError {
    TrialError::NoCrossing {
        method_id: method_id.to_string(),
        parameter: parameter.to_string(),
        value,
        search_bound_seconds: bound_seconds,
    }
}

/// Onset by a noise relative threshold on a quiet standing window.
///
/// The reference and the dispersion are arguments rather than being derived here,
/// because one registered rule builds its threshold from the raw window standard
/// deviation and then tests it against a filtered trace. That asymmetry is in the
/// code it comes from and in none of its publications, and a signature that computed
/// both from one signal could not express it.
#[allow(clippy::too_many_arguments)]
pub fn onset_noise_relative(
    tested_signal: &[f64],
    reference_newtons: f64,
    dispersion_newtons: f64,
    multiplier: f64,
    band_sides: BandSides,
    degenerate: DegenerateBandPolicy,
    search: &CrossingSearch,
    sample_rate_hz: f64,
) -> Result<usize, TrialError> {
    let half_width = multiplier * dispersion_newtons;
    let lower = reference_newtons - half_width;
    let collapsed = dispersion_newtons <= 0.0 || lower <= 0.0;
    let band = match (collapsed, degenerate) {
        (true, DegenerateBandPolicy::Refuse) => {
            return Err(TrialError::CollapsedBand {
                method_id: method_ids::ONSET_THRESHOLD_NOISE_RELATIVE.to_string(),
                parameter: "k".to_string(),
                value: multiplier,
                dispersion_newtons,
                threshold_newtons: lower,
            })
        }
        (true, DegenerateBandPolicy::FractionOfReference(fraction)) => {
            ExcursionBand::below(fraction * reference_newtons)
        }
        _ => match band_sides {
            BandSides::BelowOnly => ExcursionBand::below(lower),
            BandSides::BothSides => ExcursionBand::centred(reference_newtons, half_width),
        },
    };
    sustained_excursion(tested_signal, &band, search).ok_or_else(|| {
        no_crossing(
            method_ids::ONSET_THRESHOLD_NOISE_RELATIVE,
            "k",
            multiplier,
            search.end_index as f64 / sample_rate_hz,
        )
    })
}

/// Whether a noise relative rule watches for unloading only, or for any departure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BandSides {
    BelowOnly,
    BothSides,
}

/// What a noise relative rule does when its band collapses.
///
/// A band collapses when the quiet window has no measurable spread, which happens on
/// this corpus whenever the plate held one bit-identical value for the whole window,
/// and when the resulting threshold falls at or below zero, which happens when the
/// athlete was still moving during the window the rule assumed was quiet.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DegenerateBandPolicy {
    /// Refuse. A band of k times zero does not separate movement from noise, and any
    /// crossing it reports is decided by the last bits of the arithmetic.
    Refuse,
    /// Search against the collapsed band as stated.
    UseAsStated,
    /// Replace the threshold with a fixed fraction of the standing reference.
    FractionOfReference(f64),
}

/// Onset as the start of the last excursion before a bound, found by walking back to
/// the last sample still inside the band.
///
/// The same published rule as `onset_noise_relative` read the other way round, and not
/// reachable from it by any parameter: a forward search selects the first sample
/// outside the band, this selects the sample after the last one inside it. Given the
/// same band and the same trials, the two land on the same sample on 195 of 244 and
/// differ by up to 678 ms on the rest. Bounding the walk at the unweighting trough is
/// what keeps it off the propulsion phase, where force re-enters the band on its way
/// up.
#[allow(clippy::too_many_arguments)]
pub fn onset_final_departure_from_band(
    signal: &[f64],
    reference_newtons: f64,
    dispersion_newtons: f64,
    multiplier: f64,
    band_sides: BandSides,
    degenerate: DegenerateBandPolicy,
    search_end_index: usize,
    sample_rate_hz: f64,
) -> Result<usize, TrialError> {
    const METHOD: &str = method_ids::ONSET_THRESHOLD_NOISE_RELATIVE_FINAL_DEPARTURE;
    let half_width = multiplier * dispersion_newtons;
    let lower = reference_newtons - half_width;
    let band = match (dispersion_newtons <= 0.0 || lower <= 0.0, degenerate) {
        (true, DegenerateBandPolicy::Refuse) => {
            return Err(TrialError::CollapsedBand {
                method_id: METHOD.to_string(),
                parameter: "k".to_string(),
                value: multiplier,
                dispersion_newtons,
                threshold_newtons: lower,
            })
        }
        (true, DegenerateBandPolicy::FractionOfReference(fraction)) => {
            ExcursionBand::below(fraction * reference_newtons)
        }
        _ => match band_sides {
            BandSides::BelowOnly => ExcursionBand::below(lower),
            BandSides::BothSides => ExcursionBand::centred(reference_newtons, half_width),
        },
    };
    let bound = (search_end_index + 1).min(signal.len());
    let last_quiet = last_sample_inside_band(signal, &band, 0, bound)
        .ok_or_else(|| no_crossing(METHOD, "k", multiplier, bound as f64 / sample_rate_hz))?;
    Ok((last_quiet + 1).min(bound.saturating_sub(1)))
}

/// Onset as a fraction below the standing reference, with no reference to noise.
pub fn onset_relative_to_reference(
    signal: &[f64],
    reference_newtons: f64,
    fraction_below: f64,
    search: &CrossingSearch,
    sample_rate_hz: f64,
) -> Result<usize, TrialError> {
    let band = ExcursionBand::below((1.0 - fraction_below) * reference_newtons);
    sustained_excursion(signal, &band, search).ok_or_else(|| {
        no_crossing(
            method_ids::ONSET_THRESHOLD_RELATIVE_TO_SYSTEM_WEIGHT,
            "fraction_below",
            fraction_below,
            search.end_index as f64 / sample_rate_hz,
        )
    })
}

/// Move an index earlier by a fixed offset, reporting when the offset ran off the
/// front of the recording rather than clamping silently.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BacktrackOutcome {
    pub index: usize,
    pub clamped_at_start: bool,
}

pub fn backtrack(index: usize, offset_samples: usize) -> BacktrackOutcome {
    BacktrackOutcome {
        index: index.saturating_sub(offset_samples),
        clamped_at_start: offset_samples > index,
    }
}

/// What a last-crossing onset rule does once it has found its candidate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostCrossingRule {
    /// Walk on to the last sample at or above the standing reference itself.
    ToReferenceCrossing,
    /// Step back a fixed number of samples.
    FixedOffset(usize),
}

/// Onset as the last sample still inside the noise band before the force minimum,
/// with a look back that rejects a candidate preceded by a loading spike.
///
/// The look back exists because an athlete who preloads before unloading crosses the
/// upper band first, and the last-crossing rule would otherwise place onset after that
/// preload rather than before it.
#[allow(clippy::too_many_arguments)]
pub fn onset_last_sample_within_noise_band(
    signal: &[f64],
    reference_newtons: f64,
    dispersion_newtons: f64,
    multiplier: f64,
    search_end_index: usize,
    inverse_lookback_samples: usize,
    post_crossing: PostCrossingRule,
    sample_rate_hz: f64,
) -> Result<BacktrackOutcome, TrialError> {
    let half_width = multiplier * dispersion_newtons;
    let lower_threshold = reference_newtons - half_width;
    let upper_threshold = reference_newtons + half_width;
    let scan_end = (search_end_index + 1).min(signal.len());

    // A band of zero width admits every sample, so the last one inside it is the sample
    // before the bound whatever the athlete did. `onset_noise_relative` reads the same
    // collapse off the same two numbers and declines, and the two rules share a registry
    // entry, so a reader picking between them would otherwise get a refusal from one and a
    // converter artefact from the other.
    if dispersion_newtons <= 0.0 || lower_threshold <= 0.0 {
        return Err(TrialError::CollapsedBand {
            method_id: method_ids::ONSET_THRESHOLD_NOISE_RELATIVE.to_string(),
            parameter: "k".to_string(),
            value: multiplier,
            dispersion_newtons,
            threshold_newtons: lower_threshold,
        });
    }

    let candidate = (0..scan_end)
        .rev()
        .find(|&index| signal[index] >= lower_threshold)
        .ok_or_else(|| {
            no_crossing(
                method_ids::ONSET_THRESHOLD_NOISE_RELATIVE,
                "k",
                multiplier,
                scan_end as f64 / sample_rate_hz,
            )
        })?;

    let lookback_start = candidate.saturating_sub(inverse_lookback_samples);
    let preload_present = lookback_start < candidate
        && signal[lookback_start..=candidate]
            .iter()
            .any(|&value| value > upper_threshold);

    let (scan_limit, acceptance) = if preload_present {
        let peak = lookback_start
            + index_of_maximum(&signal[lookback_start..=candidate]).unwrap_or_default();
        match post_crossing {
            PostCrossingRule::ToReferenceCrossing => {
                (peak, Acceptance::AtOrBelow(reference_newtons))
            }
            PostCrossingRule::FixedOffset(_) => (peak, Acceptance::AtOrBelow(upper_threshold)),
        }
    } else {
        match post_crossing {
            PostCrossingRule::ToReferenceCrossing => {
                (candidate, Acceptance::AtOrAbove(reference_newtons))
            }
            PostCrossingRule::FixedOffset(offset) => return Ok(backtrack(candidate, offset)),
        }
    };

    let resolved = (0..(scan_limit + 1).min(signal.len()))
        .rev()
        .find(|&index| acceptance.accepts(signal[index]))
        .ok_or_else(|| {
            no_crossing(
                method_ids::ONSET_THRESHOLD_NOISE_RELATIVE,
                "k",
                multiplier,
                scan_limit as f64 / sample_rate_hz,
            )
        })?;

    Ok(match post_crossing {
        PostCrossingRule::ToReferenceCrossing => BacktrackOutcome {
            index: resolved,
            clamped_at_start: false,
        },
        PostCrossingRule::FixedOffset(offset) => backtrack(resolved, offset),
    })
}

#[derive(Debug, Clone, Copy)]
enum Acceptance {
    AtOrBelow(f64),
    AtOrAbove(f64),
}

impl Acceptance {
    fn accepts(self, value: f64) -> bool {
        match self {
            Acceptance::AtOrBelow(threshold) => value <= threshold,
            Acceptance::AtOrAbove(threshold) => value >= threshold,
        }
    }
}

/// Onset against a threshold recomputed from a trailing window at every sample.
///
/// The threshold adapts, so the rule can cross and uncross many times. It keeps the
/// last crossing before the force extremum and then walks back to the first sample of
/// that final cluster, which is the only part of the rule that makes it an onset
/// rather than a detector of the steepest part of the descent.
pub fn onset_adaptive_trailing_window(
    signal: &[f64],
    trailing_window_samples: usize,
    multiplier: f64,
    dispersion: DispersionEstimator,
    sample_rate_hz: f64,
) -> Result<usize, TrialError> {
    let extremum = index_of_minimum(signal).unwrap_or_default();
    if extremum <= trailing_window_samples + 1 {
        return Err(no_crossing(
            method_ids::ONSET_THRESHOLD_ADAPTIVE_TRAILING_WINDOW,
            "trailing_window_samples",
            trailing_window_samples as f64,
            extremum as f64 / sample_rate_hz,
        ));
    }

    let trailing_limit = |index: usize| -> f64 {
        let start = index.saturating_sub(trailing_window_samples);
        match mean_and_standard_deviation(&signal[start..=index], dispersion) {
            Some((window_mean, deviation)) => window_mean - multiplier * deviation,
            None => f64::NEG_INFINITY,
        }
    };

    // The threshold is a function of the index, so the loop reads the position and not
    // an element, and keeping the last crossing rather than the first is the rule.
    let last_crossing = (trailing_window_samples + 1..extremum)
        .rfind(|&index| signal[index] < trailing_limit(index));
    let last_crossing = last_crossing.ok_or_else(|| {
        no_crossing(
            method_ids::ONSET_THRESHOLD_ADAPTIVE_TRAILING_WINDOW,
            "k",
            multiplier,
            extremum as f64 / sample_rate_hz,
        )
    })?;

    let mut index = last_crossing - 1;
    while index > trailing_window_samples && signal[index] < trailing_limit(index) {
        index -= 1;
    }
    Ok(index + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn quiet_then_unload() -> Vec<f64> {
        let mut signal = vec![600.0; 1000];
        signal.extend(std::iter::repeat_n(400.0, 500));
        signal
    }

    #[test]
    fn persistence_rejects_a_single_sample_spike() {
        let mut signal = vec![600.0; 1000];
        signal[300] = 100.0;
        signal.extend(std::iter::repeat_n(400.0, 500));
        let band = ExcursionBand::below(500.0);
        let brief = sustained_excursion(
            &signal,
            &band,
            &CrossingSearch {
                start_index: 0,
                end_index: signal.len(),
                persistence_samples: 1,
                selection: CrossingSelection::First,
            },
        );
        let sustained = sustained_excursion(
            &signal,
            &band,
            &CrossingSearch {
                start_index: 0,
                end_index: signal.len(),
                persistence_samples: 50,
                selection: CrossingSelection::First,
            },
        );
        assert_eq!(brief, Some(300));
        assert_eq!(sustained, Some(1000));
    }

    #[test]
    fn a_search_floor_can_place_onset_later_than_the_athlete_moved() {
        let signal = quiet_then_unload();
        let band = ExcursionBand::below(500.0);
        let unfloored = sustained_excursion(
            &signal,
            &band,
            &CrossingSearch {
                start_index: 0,
                end_index: signal.len(),
                persistence_samples: 1,
                selection: CrossingSelection::First,
            },
        );
        let floored = sustained_excursion(
            &signal,
            &band,
            &CrossingSearch {
                start_index: 1200,
                end_index: signal.len(),
                persistence_samples: 1,
                selection: CrossingSelection::First,
            },
        );
        assert_eq!(unfloored, Some(1000));
        assert_eq!(floored, Some(1200));
    }

    #[test]
    fn a_two_sided_band_catches_a_preload_that_a_one_sided_band_misses() {
        let mut signal = vec![600.0; 1000];
        signal[400] = 700.0;
        signal.extend(std::iter::repeat_n(400.0, 500));
        let search = CrossingSearch {
            start_index: 0,
            end_index: signal.len(),
            persistence_samples: 1,
            selection: CrossingSelection::First,
        };
        let below = sustained_excursion(&signal, &ExcursionBand::below(550.0), &search);
        let both = sustained_excursion(&signal, &ExcursionBand::centred(600.0, 50.0), &search);
        assert_eq!(below, Some(1000));
        assert_eq!(both, Some(400));
    }

    /// The forward rule fires on the first noise excursion; the backward rule cannot,
    /// because a later return to the band overwrites the candidate. Over the measured
    /// corpus the forward rule places onset more than two seconds before takeoff on 31
    /// trials of 244 with no persistence requirement and on 2 with one, and the
    /// backward rule on none.
    #[test]
    fn the_two_readings_of_one_rule_split_on_a_noise_excursion() {
        let mut signal = vec![600.0; 1000];
        signal[300] = 520.0;
        signal.extend(std::iter::repeat_n(400.0, 500));
        let trough = signal.len() - 1;
        let forward = onset_noise_relative(
            &signal,
            600.0,
            10.0,
            5.0,
            BandSides::BothSides,
            DegenerateBandPolicy::UseAsStated,
            &CrossingSearch {
                start_index: 0,
                end_index: signal.len(),
                persistence_samples: 1,
                selection: CrossingSelection::First,
            },
            1200.0,
        )
        .unwrap();
        let backward = onset_final_departure_from_band(
            &signal,
            600.0,
            10.0,
            5.0,
            BandSides::BothSides,
            DegenerateBandPolicy::UseAsStated,
            trough,
            1200.0,
        )
        .unwrap();
        assert_eq!(forward, 300, "forward rule missed the noise excursion");
        assert_eq!(
            backward, 1000,
            "backward rule was fooled by the noise excursion"
        );
    }

    /// A persistence requirement closes most of the gap, which is why the direction is
    /// not on its own what separates a working rule from a failing one.
    #[test]
    fn persistence_brings_the_forward_reading_back_to_the_backward_one() {
        let mut signal = vec![600.0; 1000];
        signal[300] = 520.0;
        signal.extend(std::iter::repeat_n(400.0, 500));
        let forward = onset_noise_relative(
            &signal,
            600.0,
            10.0,
            5.0,
            BandSides::BothSides,
            DegenerateBandPolicy::UseAsStated,
            &CrossingSearch {
                start_index: 0,
                end_index: signal.len(),
                persistence_samples: 210,
                selection: CrossingSelection::First,
            },
            1200.0,
        )
        .unwrap();
        assert_eq!(forward, 1000);
    }

    /// The plate holds one bit-identical value for the whole 0.25 s quiet window on 11
    /// trials in 244, and the rule that consumes its standard deviation then has no
    /// band at all.
    #[test]
    fn a_quiet_window_with_no_noise_collapses_the_band_and_is_named_as_such() {
        let mut signal = vec![600.0; 1000];
        signal.extend(std::iter::repeat_n(400.0, 500));
        let search = CrossingSearch {
            start_index: 0,
            end_index: signal.len(),
            persistence_samples: 1,
            selection: CrossingSelection::First,
        };
        let refused = onset_noise_relative(
            &signal,
            600.0,
            0.0,
            5.0,
            BandSides::BelowOnly,
            DegenerateBandPolicy::Refuse,
            &search,
            1200.0,
        )
        .unwrap_err();
        let message = refused.to_string();
        assert!(
            message.contains("onset.threshold.noise_relative"),
            "{message}"
        );
        assert!(message.contains("dispersion"), "{message}");

        let fallen_back = onset_noise_relative(
            &signal,
            600.0,
            0.0,
            5.0,
            BandSides::BelowOnly,
            DegenerateBandPolicy::FractionOfReference(0.95),
            &search,
            1200.0,
        )
        .unwrap();
        assert_eq!(fallen_back, 1000);
    }

    #[test]
    fn a_rule_declines_under_the_same_id_it_succeeds_under() {
        let quiet = vec![600.0; 1200];
        let search = CrossingSearch {
            start_index: 0,
            end_index: 1199,
            persistence_samples: 1,
            selection: CrossingSelection::First,
        };
        let refused = onset_relative_to_reference(&quiet, 600.0, 0.1, &search, 1200.0).unwrap_err();
        let message = refused.to_string();
        assert!(
            message.contains(method_ids::ONSET_THRESHOLD_RELATIVE_TO_SYSTEM_WEIGHT),
            "{message}"
        );
        assert!(
            method_ids::ALL.contains(&method_ids::ONSET_THRESHOLD_RELATIVE_TO_SYSTEM_WEIGHT),
            "the id a caller is handed on failure has to be one they could have selected"
        );
        assert!(!message.contains("percent_bodyweight"), "{message}");
    }

    #[test]
    fn a_backtrack_that_runs_off_the_front_reports_that_it_clamped() {
        assert_eq!(
            backtrack(10, 36),
            BacktrackOutcome {
                index: 0,
                clamped_at_start: true
            }
        );
        assert_eq!(
            backtrack(100, 36),
            BacktrackOutcome {
                index: 64,
                clamped_at_start: false
            }
        );
    }

    #[test]
    fn onset_reports_which_method_and_parameter_failed() {
        let signal = vec![600.0; 2400];
        let error = onset_noise_relative(
            &signal,
            600.0,
            0.0,
            5.0,
            BandSides::BothSides,
            DegenerateBandPolicy::UseAsStated,
            &CrossingSearch {
                start_index: 600,
                end_index: 1800,
                persistence_samples: 1,
                selection: CrossingSelection::First,
            },
            1200.0,
        )
        .unwrap_err();
        let message = error.to_string();
        assert!(
            message.contains("onset.threshold.noise_relative"),
            "{message}"
        );
        assert!(message.contains("k = 5"), "{message}");
    }
}
