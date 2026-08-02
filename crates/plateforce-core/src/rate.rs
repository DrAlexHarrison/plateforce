//! Rate of force development, as chords across the force trace.
//!
//! Three published rules are one arithmetic at three anchorings: a chord from onset over a
//! stated epoch, consecutive chords of a stated width laid end to end, and the steepest
//! chord anywhere inside a window. A two-sample centred difference is the steepest-chord
//! rule at a width of two sample intervals, so it is a width here rather than a fourth
//! function.
//!
//! Nothing here decides a method. A caller passes the width and the interval a bound rule
//! resolved and gets back the chord, including where it was taken, so the record can say
//! which part of the trace produced the number.

/// A straight line between two samples of the force trace.
///
/// The indices travel with the value because two rules at the same width can report the
/// same rate from different parts of the trace, and a reader comparing them needs to see
/// which span each one measured.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Chord {
    pub start_index: usize,
    pub end_index: usize,
    pub force_change_newtons: f64,
    pub elapsed_seconds: f64,
}

impl Chord {
    pub fn rate_newtons_per_second(&self) -> f64 {
        self.force_change_newtons / self.elapsed_seconds
    }

    /// The force at the far end of the chord, which one published rule reports beside the
    /// rate and which is a different quantity under a gross convention than under a net
    /// one.
    pub fn force_at_end_newtons(&self, values: &[f64]) -> f64 {
        values[self.end_index]
    }
}

/// The chord between two named samples, or nothing when the span is empty or runs off the
/// end of the trace.
pub fn chord(
    values: &[f64],
    start_index: usize,
    end_index: usize,
    sample_interval_seconds: f64,
) -> Option<Chord> {
    if end_index <= start_index || end_index >= values.len() {
        return None;
    }
    Some(Chord {
        start_index,
        end_index,
        force_change_newtons: values[end_index] - values[start_index],
        elapsed_seconds: (end_index - start_index) as f64 * sample_interval_seconds,
    })
}

/// Chords of one width laid end to end from a starting sample, none overlapping.
///
/// The last partial window is dropped rather than reported over a shorter span, because a
/// rate taken over a different interval than the ones beside it is not comparable with
/// them and reporting it under the same width would say it was.
pub fn sequential_chords(
    values: &[f64],
    from_index: usize,
    width_samples: usize,
    until_index: usize,
    sample_interval_seconds: f64,
) -> Vec<Chord> {
    if width_samples == 0 {
        return Vec::new();
    }
    let last = until_index.min(values.len().saturating_sub(1));
    let mut chords = Vec::new();
    let mut start = from_index;
    while start + width_samples <= last {
        if let Some(found) = chord(
            values,
            start,
            start + width_samples,
            sample_interval_seconds,
        ) {
            chords.push(found);
        }
        start += width_samples;
    }
    chords
}

/// The steepest chord of one width whose whole span lies inside the search interval.
///
/// `search_end` is the last sample the chord may reach, so a window bound decided by an
/// analysis-window rule keeps the search off the landing. Ties go to the earliest span,
/// stated because a tie broken the other way moves the reported index on a trace with a
/// flat maximum.
pub fn steepest_chord(
    values: &[f64],
    width_samples: usize,
    search_start: usize,
    search_end: usize,
    sample_interval_seconds: f64,
) -> Option<Chord> {
    if width_samples == 0 {
        return None;
    }
    let last = search_end.min(values.len().saturating_sub(1));
    let mut steepest: Option<Chord> = None;
    let mut start = search_start;
    while start + width_samples <= last {
        if let Some(candidate) = chord(
            values,
            start,
            start + width_samples,
            sample_interval_seconds,
        ) {
            let better = match steepest {
                Some(best) => candidate.force_change_newtons > best.force_change_newtons,
                None => true,
            };
            if better {
                steepest = Some(candidate);
            }
        }
        start += 1;
    }
    steepest
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_INTERVAL_SECONDS: f64 = 1.0 / 1200.0;

    /// A straight line of known slope, so the rate is known exactly rather than to a
    /// tolerance the test itself chose.
    fn ramp(slope_newtons_per_second: f64, sample_count: usize) -> Vec<f64> {
        (0..sample_count)
            .map(|index| slope_newtons_per_second * index as f64 * SAMPLE_INTERVAL_SECONDS)
            .collect()
    }

    #[test]
    fn a_chord_across_a_ramp_returns_the_ramps_slope() {
        let values = ramp(4500.0, 1200);
        let found = chord(&values, 100, 340, SAMPLE_INTERVAL_SECONDS).unwrap();
        assert!((found.rate_newtons_per_second() - 4500.0).abs() < 1e-9);
        assert!((found.elapsed_seconds - 0.2).abs() < 1e-12);
    }

    #[test]
    fn a_chord_that_runs_off_the_end_returns_nothing_rather_than_a_shorter_one() {
        let values = ramp(4500.0, 500);
        assert!(chord(&values, 400, 500, SAMPLE_INTERVAL_SECONDS).is_none());
        assert!(chord(&values, 400, 400, SAMPLE_INTERVAL_SECONDS).is_none());
    }

    #[test]
    fn sequential_chords_do_not_overlap_and_drop_the_partial_window() {
        let values = ramp(1000.0, 1000);
        let chords = sequential_chords(&values, 0, 300, 999, SAMPLE_INTERVAL_SECONDS);
        assert_eq!(chords.len(), 3);
        for (index, found) in chords.iter().enumerate() {
            assert_eq!(found.start_index, index * 300);
            assert_eq!(found.end_index, (index + 1) * 300);
            assert!((found.rate_newtons_per_second() - 1000.0).abs() < 1e-9);
        }
    }

    /// One triangle of known steepness inside an otherwise flat trace, so the steepest
    /// chord has a known value and a known location.
    fn one_steep_stretch() -> Vec<f64> {
        let mut values = vec![600.0f64; 2400];
        for index in 1000..1240 {
            values[index] = 600.0 + 3000.0 * (index - 1000) as f64 * SAMPLE_INTERVAL_SECONDS;
        }
        for value in values.iter_mut().skip(1240) {
            *value = 600.0 + 3000.0 * 239.0 * SAMPLE_INTERVAL_SECONDS;
        }
        values
    }

    #[test]
    fn the_steepest_chord_finds_the_steep_stretch_and_reports_where_it_was() {
        let values = one_steep_stretch();
        let found =
            steepest_chord(&values, 24, 0, values.len() - 1, SAMPLE_INTERVAL_SECONDS).unwrap();
        assert!((found.rate_newtons_per_second() - 3000.0).abs() < 1e-9);
        assert!((1000..1240).contains(&found.start_index), "{found:?}");
    }

    /// The window bound is the whole point of the search interval: a later, steeper
    /// stretch outside it must not be the answer.
    #[test]
    fn the_search_interval_bounds_the_steepest_chord() {
        let mut values = vec![600.0f64; 2400];
        for index in 1200..2400 {
            values[index] = 600.0 + 9000.0 * (index - 1200) as f64 * SAMPLE_INTERVAL_SECONDS;
        }
        let inside = steepest_chord(&values, 24, 0, 1100, SAMPLE_INTERVAL_SECONDS).unwrap();
        assert_eq!(inside.rate_newtons_per_second(), 0.0);
        let across = steepest_chord(&values, 24, 0, 2399, SAMPLE_INTERVAL_SECONDS).unwrap();
        assert!((across.rate_newtons_per_second() - 9000.0).abs() < 1e-9);
    }

    /// The registry states that a two-sample centred difference is this rule at twice the
    /// sample interval rather than a method of its own, so the two agree by construction
    /// and this holds them to it.
    #[test]
    fn a_width_of_two_samples_is_the_centred_difference() {
        let values = one_steep_stretch();
        let width_two =
            steepest_chord(&values, 2, 0, values.len() - 1, SAMPLE_INTERVAL_SECONDS).unwrap();
        let centred = (0..values.len() - 2)
            .map(|index| (values[index + 2] - values[index]) / (2.0 * SAMPLE_INTERVAL_SECONDS))
            .fold(f64::NEG_INFINITY, f64::max);
        assert!((width_two.rate_newtons_per_second() - centred).abs() < 1e-9);
    }

    #[test]
    fn a_width_of_zero_returns_nothing_rather_than_dividing_by_zero() {
        let values = ramp(1000.0, 500);
        assert!(steepest_chord(&values, 0, 0, 499, SAMPLE_INTERVAL_SECONDS).is_none());
        assert!(sequential_chords(&values, 0, 0, 499, SAMPLE_INTERVAL_SECONDS).is_empty());
    }
}
