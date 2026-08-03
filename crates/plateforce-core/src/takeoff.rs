//! Takeoff, and the flight phase selection that decides it.
//!
//! Five registered rules answer this question. Two of them select the longest run of
//! low force in the recording, which is the jump only if the recording was trimmed to
//! one jump. On an untrimmed recording the longest low force run is often the athlete
//! standing off the plate afterwards, so the selection is reported rather than
//! returned bare.

use crate::method_ids;
use crate::signal::TrialError;
use crate::statistics::{index_of_minimum, mean_and_standard_deviation, DispersionEstimator};

/// Whether a residual force threshold is applied to the signed value or its magnitude.
///
/// An unloaded plate can read negative. Under the signed comparison such a sample is
/// below any positive threshold and counts as flight; under the magnitude comparison
/// it does not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualComparison {
    SignedValue,
    Magnitude,
}

impl ResidualComparison {
    fn is_below(self, value_newtons: f64, threshold_newtons: f64) -> bool {
        match self {
            ResidualComparison::SignedValue => value_newtons < threshold_newtons,
            ResidualComparison::Magnitude => value_newtons.abs() < threshold_newtons,
        }
    }
}

/// A contiguous run of samples below a threshold.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LowForceRun {
    pub start_index: usize,
    pub end_index: usize,
}

impl LowForceRun {
    pub fn length(&self) -> usize {
        self.end_index - self.start_index + 1
    }
}

/// Which run a longest-run rule chose, and what it chose against.
///
/// `selected_is_first_qualifying` is false whenever the rule preferred a later run to
/// the first one long enough to be a flight phase. On this corpus that happens on 155
/// of 244 trials, and the two tools it reproduces report nothing when it does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FlightPhaseSelection {
    pub start_index: usize,
    pub end_index: usize,
    pub qualifying_run_count: usize,
    pub first_qualifying_start_index: usize,
    pub selected_is_first_qualifying: bool,
}

/// Every contiguous run below the threshold.
pub fn low_force_runs(
    signal: &[f64],
    threshold_newtons: f64,
    comparison: ResidualComparison,
) -> Vec<LowForceRun> {
    let mut runs = Vec::new();
    let mut open: Option<usize> = None;
    for (index, &value) in signal.iter().enumerate() {
        if comparison.is_below(value, threshold_newtons) {
            open.get_or_insert(index);
        } else if let Some(start) = open.take() {
            runs.push(LowForceRun {
                start_index: start,
                end_index: index - 1,
            });
        }
    }
    if let Some(start) = open {
        runs.push(LowForceRun {
            start_index: start,
            end_index: signal.len() - 1,
        });
    }
    runs
}

/// Takeoff as the start of the first run below the threshold that lasts long enough.
pub fn takeoff_first_sustained_run(
    signal: &[f64],
    threshold_newtons: f64,
    minimum_flight_samples: usize,
    comparison: ResidualComparison,
    search_from: usize,
    sample_rate_hz: f64,
) -> Result<usize, TrialError> {
    let mut run_start: Option<usize> = None;
    for (index, &value) in signal.iter().enumerate().skip(search_from) {
        if comparison.is_below(value, threshold_newtons) {
            let start = *run_start.get_or_insert(index);
            if index - start + 1 >= minimum_flight_samples {
                return Ok(start);
            }
        } else {
            run_start = None;
        }
    }
    Err(TrialError::NoCrossing {
        method_id: method_ids::TAKEOFF_THRESHOLD_ABSOLUTE_FORCE.to_string(),
        parameter: "threshold_newtons".to_string(),
        value: threshold_newtons,
        search_bound_seconds: signal.len() as f64 / sample_rate_hz,
    })
}

/// How a longest-run rule treats runs shorter than the minimum flight time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShortRunHandling {
    /// Compare every run by length, then check the winner against the minimum. A short
    /// run can win and disqualify the trial even when a valid flight phase exists.
    RankThenFilter,
    /// Discard short runs first, then compare what is left.
    FilterThenRank,
}

/// Takeoff as the start of the longest run below the threshold.
pub fn takeoff_longest_run(
    signal: &[f64],
    threshold_newtons: f64,
    minimum_flight_samples: usize,
    comparison: ResidualComparison,
    short_run_handling: ShortRunHandling,
    sample_rate_hz: f64,
) -> Result<FlightPhaseSelection, TrialError> {
    let runs = low_force_runs(signal, threshold_newtons, comparison);
    let qualifying: Vec<LowForceRun> = runs
        .iter()
        .copied()
        .filter(|run| run.length() >= minimum_flight_samples)
        .collect();

    let failure = || TrialError::NoCrossing {
        method_id: method_ids::TAKEOFF_THRESHOLD_ABSOLUTE_FORCE.to_string(),
        parameter: "minimum_flight_samples".to_string(),
        value: minimum_flight_samples as f64,
        search_bound_seconds: signal.len() as f64 / sample_rate_hz,
    };

    let pool = match short_run_handling {
        ShortRunHandling::RankThenFilter => &runs,
        ShortRunHandling::FilterThenRank => &qualifying,
    };
    let longest = pool
        .iter()
        .copied()
        .reduce(|best, run| {
            if run.length() > best.length() {
                run
            } else {
                best
            }
        })
        .ok_or_else(failure)?;
    if longest.length() < minimum_flight_samples {
        return Err(failure());
    }

    let first_qualifying = qualifying.first().copied().unwrap_or(longest);
    Ok(FlightPhaseSelection {
        start_index: longest.start_index,
        end_index: longest.end_index,
        qualifying_run_count: qualifying.len(),
        first_qualifying_start_index: first_qualifying.start_index,
        selected_is_first_qualifying: longest.start_index == first_qualifying.start_index,
    })
}

/// Takeoff as the sample before a descending crossing that stays below.
///
/// This differs from a sustained run rule in what it returns: the crossing index is
/// the last sample at or above the threshold, so takeoff is placed one sample before
/// the force is inside the flight band.
pub fn takeoff_descending_crossing(
    signal: &[f64],
    threshold_newtons: f64,
    consecutive_samples: usize,
    sample_rate_hz: f64,
) -> Result<usize, TrialError> {
    // The test is on a pair of adjacent samples and then on the window after them, so
    // the index is the subject of the loop rather than a way of reaching one element.
    #[allow(clippy::needless_range_loop)]
    for index in 1..signal.len() {
        if !(signal[index - 1] >= threshold_newtons && signal[index] < threshold_newtons) {
            continue;
        }
        let confirm_start = index + 1;
        let confirm_end = (confirm_start + consecutive_samples).min(signal.len());
        if confirm_end - confirm_start < consecutive_samples {
            continue;
        }
        if signal[confirm_start..confirm_end]
            .iter()
            .all(|&value| value < threshold_newtons)
        {
            return Ok(index);
        }
    }
    Err(TrialError::NoCrossing {
        method_id: method_ids::TAKEOFF_THRESHOLD_DESCENDING_CROSSING.to_string(),
        parameter: "threshold_newtons".to_string(),
        value: threshold_newtons,
        search_bound_seconds: signal.len() as f64 / sample_rate_hz,
    })
}

/// A flight threshold re-derived from the trial's own flight phase, and the takeoff it
/// then places.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReestimatedFlight {
    pub takeoff_index: usize,
    pub provisional_takeoff_index: usize,
    pub threshold_newtons: f64,
}

/// Takeoff against a threshold estimated from the middle of a provisional flight phase.
///
/// The provisional pass finds a flight window with a fixed residual threshold, the
/// middle fraction of that window supplies the plate's actual unloaded mean and
/// spread, and takeoff is redetected against a threshold built from those. It answers
/// the objection that a fixed newton threshold means different things on different
/// plates.
#[allow(clippy::too_many_arguments)]
pub fn takeoff_reestimated_flight_threshold(
    signal: &[f64],
    search_from: usize,
    provisional_threshold_newtons: f64,
    trim_fraction: f64,
    multiplier: f64,
    dispersion: DispersionEstimator,
    sample_rate_hz: f64,
) -> Result<ReestimatedFlight, TrialError> {
    // The id the caller selected, so the rule reports the same name whether it worked or
    // not. Reported under a second name, one rule looks like two and the failing one
    // resolves nowhere.
    let failure = || TrialError::NoCrossing {
        method_id: method_ids::TAKEOFF_THRESHOLD_FLIGHT_NOISE_K_SD.to_string(),
        parameter: "provisional_threshold_newtons".to_string(),
        value: provisional_threshold_newtons,
        search_bound_seconds: signal.len() as f64 / sample_rate_hz,
    };
    let tail = signal.get(search_from..).ok_or_else(failure)?;
    let provisional = search_from
        + tail
            .iter()
            .position(|&value| value <= provisional_threshold_newtons)
            .ok_or_else(failure)?;
    let last_low = search_from
        + tail
            .iter()
            .rposition(|&value| value <= provisional_threshold_newtons)
            .ok_or_else(failure)?;

    let unchanged = ReestimatedFlight {
        takeoff_index: provisional,
        provisional_takeoff_index: provisional,
        threshold_newtons: provisional_threshold_newtons,
    };
    let trim = (trim_fraction * (last_low - provisional) as f64) as usize;
    let (middle_start, middle_end) = (provisional + trim, last_low.saturating_sub(trim));
    if middle_end <= middle_start || middle_end - middle_start < 2 {
        return Ok(unchanged);
    }
    let Some((flight_mean, flight_deviation)) =
        mean_and_standard_deviation(&signal[middle_start..middle_end], dispersion)
    else {
        return Ok(unchanged);
    };

    let threshold = flight_mean + multiplier * flight_deviation;
    let redetected = signal[search_from..]
        .iter()
        .position(|&value| value <= threshold)
        .map(|offset| search_from + offset);
    Ok(ReestimatedFlight {
        takeoff_index: redetected.unwrap_or(provisional),
        provisional_takeoff_index: provisional,
        threshold_newtons: threshold,
    })
}

/// The force minimum, which several rules use as a search bound rather than as a
/// landmark in its own right.
///
/// The bound matters more than it looks: scoped to takeoff the minimum is the last
/// sample before the plate unloads, because force is still falling there. Scoped to
/// the propulsive peak it is the countermovement dip that was meant.
pub fn force_minimum_index(signal: &[f64], search_end: usize) -> Option<usize> {
    let end = search_end.min(signal.len());
    if end == 0 {
        return None;
    }
    index_of_minimum(&signal[..end])
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Quiet standing, a brief dip that is not flight, then the real flight phase, then
    /// a longer unloaded stretch after the athlete steps off.
    fn untrimmed_recording() -> Vec<f64> {
        let mut signal = vec![600.0; 1000];
        signal.extend(std::iter::repeat_n(0.0, 20));
        signal.extend(std::iter::repeat_n(600.0, 200));
        signal.extend(std::iter::repeat_n(0.0, 400));
        signal.extend(std::iter::repeat_n(600.0, 300));
        signal.extend(std::iter::repeat_n(0.0, 900));
        signal
    }

    #[test]
    fn a_brief_dip_is_not_taken_for_a_flight_phase() {
        let signal = untrimmed_recording();
        let takeoff = takeoff_first_sustained_run(
            &signal,
            20.0,
            100,
            ResidualComparison::SignedValue,
            0,
            1000.0,
        )
        .unwrap();
        assert_eq!(takeoff, 1220);
    }

    #[test]
    fn the_longest_run_rule_lands_after_the_athlete_has_already_landed() {
        let signal = untrimmed_recording();
        let selection = takeoff_longest_run(
            &signal,
            20.0,
            100,
            ResidualComparison::SignedValue,
            ShortRunHandling::RankThenFilter,
            1000.0,
        )
        .unwrap();
        assert_eq!(selection.start_index, 1920, "picked the post-landing quiet");
        assert_eq!(selection.qualifying_run_count, 2);
        assert_eq!(selection.first_qualifying_start_index, 1220);
        assert!(
            !selection.selected_is_first_qualifying,
            "silent misplacement"
        );
    }

    #[test]
    fn a_negative_reading_on_an_unloaded_plate_splits_the_two_comparisons() {
        let mut signal = vec![600.0; 100];
        signal.extend(std::iter::repeat_n(-15.0, 200));
        let signed = takeoff_first_sustained_run(
            &signal,
            10.0,
            100,
            ResidualComparison::SignedValue,
            0,
            1000.0,
        );
        let magnitude = takeoff_first_sustained_run(
            &signal,
            10.0,
            100,
            ResidualComparison::Magnitude,
            0,
            1000.0,
        );
        assert_eq!(signed.unwrap(), 100);
        assert!(magnitude.is_err(), "magnitude rule accepted a 15 N reading");
    }

    #[test]
    fn short_run_handling_changes_whether_the_trial_is_usable_at_all() {
        let mut signal = vec![600.0; 100];
        signal.extend(std::iter::repeat_n(0.0, 30));
        signal.extend(std::iter::repeat_n(600.0, 100));
        signal.extend(std::iter::repeat_n(0.0, 25));
        signal.extend(std::iter::repeat_n(600.0, 100));
        let ranked = takeoff_longest_run(
            &signal,
            20.0,
            20,
            ResidualComparison::SignedValue,
            ShortRunHandling::RankThenFilter,
            1000.0,
        );
        assert_eq!(ranked.unwrap().start_index, 100);
        let filtered = takeoff_longest_run(
            &signal,
            20.0,
            28,
            ResidualComparison::SignedValue,
            ShortRunHandling::FilterThenRank,
            1000.0,
        );
        assert_eq!(filtered.unwrap().start_index, 100);
    }

    #[test]
    fn a_takeoff_that_is_never_reached_names_the_threshold_it_wanted() {
        let signal = vec![600.0; 500];
        let error = takeoff_first_sustained_run(
            &signal,
            10.0,
            100,
            ResidualComparison::Magnitude,
            0,
            1200.0,
        )
        .unwrap_err();
        let message = error.to_string();
        assert!(
            message.contains("takeoff.threshold.absolute_force"),
            "{message}"
        );
        assert!(message.contains("threshold_newtons"), "{message}");
    }
}

/// Telling a landing apart from the reweighting that leads into propulsion, by the shape of
/// the rise out of a low-force run.
///
/// Both shipped takeoff rules pick a stretch of near-zero force and call it flight. The
/// first-run rule is destroyed by an athlete who unloads the plate without leaving it; the
/// longest-run rule is destroyed by an untrimmed recording, where the athlete standing off
/// the plate afterwards outlasts the jump. Neither looks at what happens next, and that is
/// where the answer is: force leaves a genuine flight phase through a collision, and a false
/// one through the athlete's own push.
pub mod landing_shape {
    /// The settings the registry entry `takeoff.threshold.landing_shape` declares.
    #[derive(Debug, Clone, Copy, PartialEq)]
    pub struct LandingShapeSpec {
        pub landing_rise_rate_floor_bodyweights_per_second: f64,
        pub landing_peak_floor_bodyweights: f64,
        pub maximum_rise_seconds: f64,
        pub slope_span_seconds: f64,
        pub minimum_run_seconds: f64,
        pub bridged_gap_seconds: f64,
        pub bridged_gap_ceiling_bodyweights: f64,
    }

    /// The values fitted on the 244-trial corpus, matching the registry row's defaults.
    impl Default for LandingShapeSpec {
        fn default() -> Self {
            Self {
                landing_rise_rate_floor_bodyweights_per_second: 20.0,
                landing_peak_floor_bodyweights: 0.5,
                maximum_rise_seconds: 0.600,
                slope_span_seconds: 0.010,
                minimum_run_seconds: 0.008,
                bridged_gap_seconds: 0.020,
                bridged_gap_ceiling_bodyweights: 0.10,
            }
        }
    }

    /// The three numbers read off the rise out of a run, all of them scale free.
    #[derive(Debug, Clone, Copy, PartialEq)]
    pub struct RiseShape {
        /// Concavity as one number. Computed and reported, and deliberately not used to
        /// decide: on this corpus flight runs and other runs both sit at 0.437.
        pub rise_fullness: f64,
        pub peak_rise_rate_bodyweights_per_second: f64,
        pub peak_bodyweights: f64,
        pub rise_seconds: f64,
        pub peak_sample: usize,
    }

    #[derive(Debug, Clone, Copy, PartialEq)]
    pub struct LowForceRun {
        pub start_sample: usize,
        pub end_sample: usize,
        pub duration_seconds: f64,
        /// No rise follows, so nothing in the signal says whether the recording stopped
        /// mid-flight or the athlete walked off. The verdict is absent rather than false.
        pub ends_the_recording: bool,
        pub shape: Option<RiseShape>,
        pub is_flight: bool,
    }

    /// Every stretch below the threshold, as half-open sample ranges, with noise gaps bridged.
    pub fn low_force_runs(
        vertical_force_newtons: &[f64],
        threshold_newtons: f64,
        system_weight_newtons: f64,
        sample_rate_hz: f64,
        spec: &LandingShapeSpec,
    ) -> Vec<(usize, usize)> {
        let mut raw: Vec<(usize, usize)> = Vec::new();
        let mut run_start: Option<usize> = None;
        for (sample, value) in vertical_force_newtons.iter().enumerate() {
            let is_below = *value < threshold_newtons;
            match (is_below, run_start) {
                (true, None) => run_start = Some(sample),
                (false, Some(start)) => {
                    raw.push((start, sample));
                    run_start = None;
                }
                _ => {}
            }
        }
        if let Some(start) = run_start {
            raw.push((start, vertical_force_newtons.len()));
        }

        let bridge_samples = (spec.bridged_gap_seconds * sample_rate_hz) as usize;
        let ceiling_newtons = spec.bridged_gap_ceiling_bodyweights * system_weight_newtons.max(0.0);
        let mut merged: Vec<(usize, usize)> = Vec::new();
        for (start_sample, end_sample) in raw {
            if let Some(&(previous_start, previous_end)) = merged.last() {
                let gap = &vertical_force_newtons[previous_end..start_sample];
                let gap_stays_small = gap.is_empty()
                    || gap.iter().copied().fold(f64::NEG_INFINITY, f64::max) <= ceiling_newtons;
                if start_sample - previous_end <= bridge_samples && gap_stays_small {
                    *merged.last_mut().expect("just read") = (previous_start, end_sample);
                    continue;
                }
            }
            merged.push((start_sample, end_sample));
        }

        let minimum_samples = ((spec.minimum_run_seconds * sample_rate_hz) as usize).max(1);
        merged
            .into_iter()
            .filter(|(start, end)| end - start >= minimum_samples)
            .collect()
    }

    /// A centred moving average, zero-padded at both ends, matching the reference
    /// implementation's convolution so the two place takeoff on the same sample.
    fn centred_moving_average(segment: &[f64], span: usize) -> Vec<f64> {
        let lead = (span - 1) / 2;
        (0..segment.len())
            .map(|centre| {
                let total: f64 = (0..span)
                    .filter_map(|offset| {
                        (centre + lead)
                            .checked_sub(offset)
                            .and_then(|index| segment.get(index))
                    })
                    .sum();
                total / span as f64
            })
            .collect()
    }

    /// The shape numbers for the rise out of a run, or nothing when there is no rise to read.
    ///
    /// The peak is the first local turnover inside the window rather than the window's
    /// maximum, so a landing followed by a second larger event is measured on its own rise.
    pub fn rise_after(
        vertical_force_newtons: &[f64],
        run_end_sample: usize,
        system_weight_newtons: f64,
        sample_rate_hz: f64,
        spec: &LandingShapeSpec,
    ) -> Option<RiseShape> {
        if system_weight_newtons <= 0.0 || run_end_sample + 2 >= vertical_force_newtons.len() {
            return None;
        }
        let window_end = vertical_force_newtons
            .len()
            .min(run_end_sample + (spec.maximum_rise_seconds * sample_rate_hz) as usize);
        let segment = &vertical_force_newtons[run_end_sample..window_end];
        if segment.len() < 4 {
            return None;
        }

        // Smoothed only to locate the turnover. Every number below is measured on the raw
        // signal, so the smoothing window cannot flatten a rate or fill a shape.
        let span = ((spec.slope_span_seconds * sample_rate_hz) as usize).max(3);
        let smoothed = centred_moving_average(segment, span);
        let slope: Vec<f64> = smoothed.windows(2).map(|pair| pair[1] - pair[0]).collect();

        let mut peak_offset = None;
        if slope.len() > 2 * span {
            for offset in span..slope.len() - span {
                if slope[offset] <= 0.0 && slope[offset..offset + span].iter().all(|s| *s <= 0.0) {
                    peak_offset = Some(offset);
                    break;
                }
            }
        }
        let peak_offset = peak_offset.unwrap_or_else(|| {
            smoothed
                .iter()
                .enumerate()
                .fold((0usize, f64::NEG_INFINITY), |best, (index, value)| {
                    if *value > best.1 {
                        (index, *value)
                    } else {
                        best
                    }
                })
                .0
        });
        if peak_offset < span || !slope[..peak_offset].iter().any(|s| *s > 0.0) {
            return None;
        }

        let rise = &segment[..=peak_offset];
        let start_newtons = rise[0];
        let peak_newtons = rise.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        if peak_newtons - start_newtons < 0.2 * system_weight_newtons {
            return None;
        }

        let height = peak_newtons - start_newtons;
        let rise_fullness = rise
            .iter()
            .map(|value| ((value - start_newtons) / height).clamp(0.0, 1.0))
            .sum::<f64>()
            / rise.len() as f64;

        let seconds_per_span = span as f64 / sample_rate_hz;
        let peak_slope = (0..peak_offset.max(1))
            .filter(|index| index + span < segment.len())
            .map(|index| (segment[index + span] - segment[index]) / seconds_per_span)
            .fold(f64::NEG_INFINITY, f64::max);

        Some(RiseShape {
            rise_fullness,
            peak_rise_rate_bodyweights_per_second: peak_slope / system_weight_newtons,
            peak_bodyweights: peak_newtons / system_weight_newtons,
            rise_seconds: peak_offset as f64 / sample_rate_hz,
            peak_sample: run_end_sample + peak_offset,
        })
    }

    /// Whether the rise out of a run is a collision rather than a muscular push.
    pub fn looks_like_a_landing(shape: Option<&RiseShape>, spec: &LandingShapeSpec) -> bool {
        shape.is_some_and(|shape| {
            shape.peak_rise_rate_bodyweights_per_second
                >= spec.landing_rise_rate_floor_bodyweights_per_second
                && shape.peak_bodyweights >= spec.landing_peak_floor_bodyweights
        })
    }

    /// Every low-force run with the shape of its exit and the verdict that follows from it.
    pub fn classify_runs(
        vertical_force_newtons: &[f64],
        system_weight_newtons: f64,
        threshold_newtons: f64,
        sample_rate_hz: f64,
        spec: &LandingShapeSpec,
    ) -> Vec<LowForceRun> {
        low_force_runs(
            vertical_force_newtons,
            threshold_newtons,
            system_weight_newtons,
            sample_rate_hz,
            spec,
        )
        .into_iter()
        .map(|(start_sample, end_sample)| {
            let ends_the_recording = end_sample >= vertical_force_newtons.len();
            let shape = if ends_the_recording {
                None
            } else {
                rise_after(
                    vertical_force_newtons,
                    end_sample,
                    system_weight_newtons,
                    sample_rate_hz,
                    spec,
                )
            };
            LowForceRun {
                start_sample,
                end_sample,
                duration_seconds: (end_sample - start_sample) as f64 / sample_rate_hz,
                ends_the_recording,
                is_flight: !ends_the_recording && looks_like_a_landing(shape.as_ref(), spec),
                shape,
            }
        })
        .collect()
    }

    /// Takeoff as the start of the first run the recording closes with a landing, and how
    /// many such runs it found.
    ///
    /// First rather than longest: once the runs that are not flights are removed, the
    /// remaining ambiguity is a recording holding more than one real jump, and the first is
    /// the one the operator asked about. The count travels so a caller can say so rather
    /// than silently reporting one of several.
    pub fn takeoff_by_landing_shape(
        vertical_force_newtons: &[f64],
        system_weight_newtons: f64,
        threshold_newtons: f64,
        sample_rate_hz: f64,
        spec: &LandingShapeSpec,
    ) -> (Option<usize>, usize) {
        let flights: Vec<LowForceRun> = classify_runs(
            vertical_force_newtons,
            system_weight_newtons,
            threshold_newtons,
            sample_rate_hz,
            spec,
        )
        .into_iter()
        .filter(|run| run.is_flight)
        .collect();
        (flights.first().map(|run| run.start_sample), flights.len())
    }
}
