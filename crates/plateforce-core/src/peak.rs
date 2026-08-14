//! The largest force in a stretch of the recording.
//!
//! Four registry entries disagree about what "peak force" means, and three of the four
//! disagree only about what the series is before the maximum is taken: raw, less system
//! weight, or smoothed. So the maximum itself is one function and the series it reads is
//! the choice.

use crate::refusal::{require_numeric_span, SamplesCarryNoNumber};
use crate::smoothing::{moving_average_boxcar, SmoothingError};

/// Why no peak could be taken.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PeakError {
    /// The span selected no samples, so there is nothing to be the largest.
    #[error("a peak was asked for over samples {start} to {end}, which selects no samples")]
    EmptySpan { start: usize, end: usize },
    /// At least one sample in the span carries no number.
    #[error("{0}")]
    SamplesCarryNoNumber(#[from] SamplesCarryNoNumber),
    /// The averaging window does not fit the series it was asked to smooth.
    #[error("{0}")]
    Smoothing(#[from] SmoothingError),
}

/// The largest value over the half-open span `[start, end)`, matching the convention
/// `Trial::integrate_newton_seconds` uses so a peak and an impulse read one window the
/// same way.
///
/// The shared numeric-span rule runs before the reduction, so `f64::max` never gets to turn
/// a sample carrying no number into a plausible maximum over the samples beside it.
pub fn maximum_over(values: &[f64], start: usize, end: usize) -> Result<f64, PeakError> {
    let end = end.min(values.len());
    if start >= end {
        return Err(PeakError::EmptySpan { start, end });
    }
    require_numeric_span(values, start, end)?;
    Ok(values[start..end]
        .iter()
        .copied()
        .fold(f64::NEG_INFINITY, f64::max))
}

/// Where the largest value sits, over the same half-open span and under the same clipping.
///
/// The span convention is stated once and both answers read it, so a rule asking where the
/// peak is and a rule asking how large it is cannot disagree about which samples they read.
/// Ties go to the earliest sample, which `index_of_maximum` fixes for every caller.
pub fn index_of_maximum_over(values: &[f64], start: usize, end: usize) -> Result<usize, PeakError> {
    let end = end.min(values.len());
    if start >= end {
        return Err(PeakError::EmptySpan { start, end });
    }
    require_numeric_span(values, start, end)?;
    crate::statistics::index_of_maximum(&values[start..end])
        .map(|offset| start + offset)
        .ok_or(PeakError::EmptySpan { start, end })
}

/// The largest value of a centred rectangular moving average over the same span.
///
/// The average is taken over the whole series and the maximum over the span, never the
/// reverse: smoothing a span in isolation gives its first and last samples an edge fit
/// computed from a window the recording does not end at, which moves the peak of a jump by
/// the width of the window.
///
/// A window of zero or one sample is the raw series, which is what the registry's
/// `averaging_window_seconds = 0` states, so it returns `maximum_over` rather than
/// refusing.
pub fn maximum_of_moving_average_over(
    values: &[f64],
    start: usize,
    end: usize,
    window_samples: usize,
) -> Result<f64, PeakError> {
    if window_samples <= 1 {
        return maximum_over(values, start, end);
    }
    let averaged = moving_average_boxcar(values, window_samples)?;
    // The smoother reads the whole recording, so that is its selected span even though the
    // maximum beneath it is bounded more narrowly. The smoothing request is checked first,
    // so a width that does not fit is still reported as the stated value that must change.
    require_numeric_span(values, 0, values.len())?;
    maximum_over(&averaged, start, end)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_maximum_is_taken_over_the_half_open_span() {
        let values = [1.0, 9.0, 2.0, 3.0];
        // The sample at `end` is outside, which is the same boundary the integral uses.
        assert_eq!(maximum_over(&values, 0, 2).unwrap(), 9.0);
        assert_eq!(maximum_over(&values, 2, 4).unwrap(), 3.0);
    }

    /// The index and the value are one answer read two ways, so a span whose maximum is at
    /// one sample cannot report the value from a different one.
    #[test]
    fn the_index_and_the_value_of_a_peak_name_the_same_sample() {
        let values = [1.0, 9.0, 2.0, 3.0, 9.0];
        for (start, end) in [(0usize, 2usize), (2, 4), (0, 5), (1, 99)] {
            let index = index_of_maximum_over(&values, start, end).unwrap();
            assert_eq!(values[index], maximum_over(&values, start, end).unwrap());
            assert!((start..end.min(values.len())).contains(&index));
        }
        // Ties go to the earliest sample, and a span holding both maxima has to prove it.
        assert_eq!(index_of_maximum_over(&values, 0, 5).unwrap(), 1);
        assert_eq!(
            index_of_maximum_over(&values, 1, 1),
            Err(PeakError::EmptySpan { start: 1, end: 1 })
        );
    }

    #[test]
    fn a_span_selecting_nothing_refuses_rather_than_returning_a_sentinel() {
        let values = [1.0, 2.0];
        assert_eq!(
            maximum_over(&values, 1, 1),
            Err(PeakError::EmptySpan { start: 1, end: 1 })
        );
        assert!(maximum_over(&[], 0, 5).is_err());
    }

    /// A span running past the end of the recording is clipped rather than refused, so a
    /// window rule that names the last sample by count still reads the samples that exist.
    #[test]
    fn a_span_running_past_the_recording_reads_what_is_there() {
        let values = [1.0, 4.0, 2.0];
        assert_eq!(maximum_over(&values, 0, 99).unwrap(), 4.0);
    }

    #[test]
    fn a_peak_refuses_every_non_finite_sample_in_its_span() {
        for (missing_at, missing_value) in [
            (0usize, f64::NAN),
            (1, f64::INFINITY),
            (2, f64::NEG_INFINITY),
        ] {
            let mut values = [1000.0, 1200.0, 1100.0];
            values[missing_at] = missing_value;
            for error in [
                maximum_over(&values, 0, values.len()).unwrap_err(),
                index_of_maximum_over(&values, 0, values.len()).unwrap_err(),
            ] {
                let PeakError::SamplesCarryNoNumber(missing) = error else {
                    panic!("the wrong peak refusal: {error:?}");
                };
                assert_eq!(missing.first_sample, missing_at);
                assert_eq!((missing.count, missing.samples_read), (1, 3));
            }
        }

        let outside = [f64::NAN, 1000.0, 1200.0, f64::INFINITY];
        assert_eq!(maximum_over(&outside, 1, 3).unwrap(), 1200.0);
        assert_eq!(index_of_maximum_over(&outside, 1, 3).unwrap(), 2);

        let two_missing = [f64::NAN, 1200.0, f64::INFINITY];
        let PeakError::SamplesCarryNoNumber(missing) =
            maximum_over(&two_missing, 0, 3).unwrap_err()
        else {
            panic!("a non-finite span returned the wrong error");
        };
        assert_eq!((missing.count, missing.samples_read), (2, 3));
    }

    /// The gap between the two estimators is the whole reason the registry files them
    /// apart, and it can only close, never reverse: an average of a window containing the
    /// maximum cannot exceed it.
    #[test]
    fn a_centred_average_never_peaks_above_the_raw_maximum() {
        let mut values = vec![600.0; 400];
        for (index, sample) in values.iter_mut().enumerate() {
            *sample += 300.0 * (index as f64 / 40.0).sin();
        }
        values[200] = 2400.0;

        let raw = maximum_over(&values, 0, values.len()).unwrap();
        for window in [3usize, 11, 51, 121] {
            let averaged =
                maximum_of_moving_average_over(&values, 0, values.len(), window).unwrap();
            assert!(
                averaged <= raw + 1e-9,
                "a {window}-sample average peaked at {averaged} above a raw maximum of {raw}"
            );
        }
        // A single-sample spike is exactly what the averaging estimator exists to not read,
        // so the gap has to be large here rather than merely non-negative.
        let averaged = maximum_of_moving_average_over(&values, 0, values.len(), 121).unwrap();
        assert!(raw - averaged > 100.0, "raw {raw}, averaged {averaged}");
    }

    /// The registry states this window in seconds and publishes zero as its default, which
    /// has to mean the raw series rather than a refusal.
    #[test]
    fn a_window_of_one_sample_or_none_is_the_raw_series() {
        let values = [1.0, 9.0, 2.0];
        let raw = maximum_over(&values, 0, 3).unwrap();
        assert_eq!(
            maximum_of_moving_average_over(&values, 0, 3, 0).unwrap(),
            raw
        );
        assert_eq!(
            maximum_of_moving_average_over(&values, 0, 3, 1).unwrap(),
            raw
        );
    }

    /// A triangular rise to a peak at sample 300 and a fall after it, so a span starting at
    /// the peak has the whole rising side outside it.
    fn peak_at_the_span_edge() -> Vec<f64> {
        (0..600)
            .map(|index| {
                let distance = (index as f64 - 300.0).abs();
                1800.0 - 3.0 * distance
            })
            .collect()
    }

    /// Smoothing the span alone gives its ends an edge fit taken from a window the
    /// recording does not end at, which moves the number. Measured rather than asserted,
    /// because the size of the difference is what decides whether the distinction matters.
    #[test]
    fn smoothing_the_whole_series_and_smoothing_the_span_alone_are_different_numbers() {
        let values = peak_at_the_span_edge();
        let (start, end) = (300, 500);

        let whole = maximum_of_moving_average_over(&values, start, end, 61).unwrap();
        let in_isolation = {
            let piece = &values[start..end];
            maximum_of_moving_average_over(piece, 0, piece.len(), 61).unwrap()
        };
        println!("whole series {whole} N, span alone {in_isolation} N");
        assert!(
            (whole - in_isolation).abs() > 1.0,
            "the two routes agreed to within 1 N at {whole}, so this guard proves nothing"
        );
    }

    /// The same distinction in its other form: a span shorter than the averaging window
    /// has no smoothed value of its own, while the same window over the whole recording
    /// does. A rule that smoothed the span alone would refuse here and report nothing.
    #[test]
    fn a_span_shorter_than_the_window_still_has_a_smoothed_peak_from_the_whole_series() {
        let values = peak_at_the_span_edge();
        assert!(maximum_of_moving_average_over(&values, 280, 320, 61).is_ok());
        assert_eq!(
            maximum_of_moving_average_over(&values[280..320], 0, 40, 61),
            Err(PeakError::Smoothing(
                SmoothingError::WindowLongerThanTrace {
                    window_length: 61,
                    sample_count: 40,
                }
            ))
        );
    }
}
