//! Rate of force development, as chords across the force trace.
//!
//! Three published rules are one arithmetic at three anchorings: a chord from onset over a
//! stated epoch, consecutive chords of a stated width laid end to end, and the steepest
//! chord anywhere inside a window. A two-sample centred difference is the steepest-chord
//! rule at a width of two sample intervals, so it is a width here rather than a fourth
//! function.
//!
//! Two further published rules anchor on a force level rather than on a time: the interval
//! between two stated levels, and the derivative where force first reaches a fraction of its
//! peak. Both need the instant a level was reached to a finer resolution than the sample
//! grid, so the crossing below carries an interpolated position and every rule that reads a
//! level reads it through that one function.
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

/// Where the trace first reached a force level, on and between the samples.
///
/// `sample_index` is the first sample at or above the level and `position` is where the
/// straight line between that sample and the one before it passes through it. The two differ
/// by up to one sample interval, which at 1200 Hz is 0.83 ms, and a rate taken over a 20 ms
/// window moves by 4 percent when its end moves that far.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LevelCrossing {
    pub sample_index: usize,
    pub position: f64,
}

impl LevelCrossing {
    pub fn seconds(&self, sample_interval_seconds: f64) -> f64 {
        self.position * sample_interval_seconds
    }
}

/// The first sample from `from_index` onward at which the trace stands at or above a level,
/// with the interpolated position beside it, or nothing when it never reaches the level.
/// A selected search span carrying any non-finite sample is an error under the shared
/// numeric-span rule in `refusal`.
///
/// A trace already at or above the level at `from_index` crossed before the search started,
/// so the position is that sample rather than an extrapolation backward into a stretch the
/// caller excluded.
pub fn first_crossing_at_or_above(
    values: &[f64],
    level: f64,
    from_index: usize,
    until_index: usize,
) -> Result<Option<LevelCrossing>, crate::refusal::SamplesCarryNoNumber> {
    let last = until_index.min(values.len().saturating_sub(1));
    if from_index > last {
        return Ok(None);
    }
    crate::refusal::require_numeric_span(values, from_index, last + 1)?;
    if values[from_index] >= level {
        return Ok(Some(LevelCrossing {
            sample_index: from_index,
            position: from_index as f64,
        }));
    }
    for index in (from_index + 1)..=last {
        if values[index] < level {
            continue;
        }
        let previous = values[index - 1];
        let rise = values[index] - previous;
        // A rise of zero cannot happen here, because the sample before is strictly below the
        // level and this one is at or above it. Guarded anyway rather than dividing.
        let fraction = if rise > 0.0 {
            (level - previous) / rise
        } else {
            0.0
        };
        return Ok(Some(LevelCrossing {
            sample_index: index,
            position: (index - 1) as f64 + fraction,
        }));
    }
    Ok(None)
}

/// The centred derivative at a position that need not be a sample, as the chord of two
/// sample intervals either side of it interpolated between its two neighbours.
///
/// The chord function is what computes it, so the two-sample centred difference has one home
/// and this is where it is read off between samples rather than a second spelling of it.
pub fn centred_derivative_at(
    values: &[f64],
    position: f64,
    sample_interval_seconds: f64,
) -> Option<f64> {
    if !position.is_finite() || position < 1.0 {
        return None;
    }
    let lower = position.floor() as usize;
    let upper = lower + 1;
    let at = |index: usize| {
        chord(values, index - 1, index + 1, sample_interval_seconds)
            .map(|found| found.rate_newtons_per_second())
    };
    let below = at(lower)?;
    match at(upper) {
        Some(above) => Some(below + (above - below) * (position - lower as f64)),
        // The last sample a centred difference can be taken at, which a position past it
        // reads as the derivative there rather than as no answer at all.
        None if position == lower as f64 => Some(below),
        None => None,
    }
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
        for (offset, value) in values.iter_mut().skip(1000).take(240).enumerate() {
            *value = 600.0 + 3000.0 * offset as f64 * SAMPLE_INTERVAL_SECONDS;
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
        for (offset, value) in values.iter_mut().skip(1200).enumerate() {
            *value = 600.0 + 9000.0 * offset as f64 * SAMPLE_INTERVAL_SECONDS;
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

    /// A ramp of known slope reaches a known level at a known time, so the interpolated
    /// position is exact rather than compared against itself.
    #[test]
    fn a_crossing_lands_between_the_samples_that_straddle_the_level() {
        let values = ramp(1200.0, 1200);
        // 1200 N/s at 1200 Hz rises exactly 1 N per sample, so 300.5 N sits half a sample
        // past index 300 and the sample at or above it is 301.
        let found = first_crossing_at_or_above(&values, 300.5, 0, 1199)
            .unwrap()
            .unwrap();
        assert_eq!(found.sample_index, 301);
        assert!((found.position - 300.5).abs() < 1e-9, "{found:?}");
        assert!((found.seconds(SAMPLE_INTERVAL_SECONDS) - 300.5 / 1200.0).abs() < 1e-12);
    }

    /// The two failures a level search has, told apart: a level the trace never reaches, and
    /// a level it stands at before the search began.
    #[test]
    fn a_level_never_reached_returns_nothing_and_one_already_held_returns_the_first_sample() {
        let values = ramp(1200.0, 1200);
        assert!(first_crossing_at_or_above(&values, 5000.0, 0, 1199)
            .unwrap()
            .is_none());
        // Bounded short of where the trace reaches it, which is a different fact from the
        // trace never reaching it and must not be answered from past the bound.
        assert!(first_crossing_at_or_above(&values, 900.0, 0, 500)
            .unwrap()
            .is_none());

        let held = first_crossing_at_or_above(&values, 100.0, 400, 1199)
            .unwrap()
            .unwrap();
        assert_eq!(held.sample_index, 400);
        assert_eq!(held.position, 400.0);
    }

    #[test]
    fn a_crossing_refuses_every_non_finite_sample_in_its_search_span() {
        let clean = ramp(2000.0, 1200);
        let crossing = first_crossing_at_or_above(&clean, 1000.0, 0, 1199)
            .unwrap()
            .unwrap();
        assert!((crossing.position - 600.0).abs() < 1e-9);

        for (missing_at, missing_value) in [
            (0usize, f64::NAN),
            (599, f64::INFINITY),
            (600, f64::NEG_INFINITY),
            (900, f64::NAN),
        ] {
            let mut interrupted = clean.clone();
            interrupted[missing_at] = missing_value;
            let missing = first_crossing_at_or_above(&interrupted, 1000.0, 0, 1199)
                .expect_err("a search span carrying no number cannot yield a crossing");
            assert_eq!(missing.first_sample, missing_at);
            assert_eq!((missing.count, missing.samples_read), (1, 1200));
        }

        let mut outside = clean;
        outside[0] = f64::NAN;
        outside[1199] = f64::INFINITY;
        let bounded = first_crossing_at_or_above(&outside, 1000.0, 1, 1000)
            .unwrap()
            .unwrap();
        assert!((bounded.position - 600.0).abs() < 1e-9);
    }

    /// On a ramp the derivative is the slope everywhere, including between samples, so the
    /// interpolation is held to a value the trace fixes rather than to its own arithmetic.
    #[test]
    fn the_centred_derivative_between_samples_is_the_slope_on_a_ramp() {
        let values = ramp(4500.0, 1200);
        for position in [1.0, 12.5, 600.25, 1198.0] {
            let found = centred_derivative_at(&values, position, SAMPLE_INTERVAL_SECONDS)
                .unwrap_or_else(|| panic!("no derivative at {position}"));
            assert!((found - 4500.0).abs() < 1e-6, "{position}: {found}");
        }
    }

    /// A bend the interpolation has to follow, so a version that read one neighbour and
    /// ignored the other would differ here and not on a ramp.
    #[test]
    fn the_centred_derivative_interpolates_between_its_two_neighbours() {
        // Flat, then a ramp, so the centred derivatives either side of the corner differ.
        let mut values = vec![0.0f64; 60];
        for (offset, value) in values.iter_mut().skip(30).enumerate() {
            *value = 1200.0 * offset as f64 * SAMPLE_INTERVAL_SECONDS;
        }
        let below = centred_derivative_at(&values, 30.0, SAMPLE_INTERVAL_SECONDS).unwrap();
        let above = centred_derivative_at(&values, 31.0, SAMPLE_INTERVAL_SECONDS).unwrap();
        let between = centred_derivative_at(&values, 30.25, SAMPLE_INTERVAL_SECONDS).unwrap();
        assert!(below < above, "the corner does not bend: {below} {above}");
        assert!(
            (between - (below + (above - below) * 0.25)).abs() < 1e-9,
            "{between} is not a quarter of the way from {below} to {above}"
        );
    }

    /// A position with no sample either side of it has no centred difference, which is
    /// nothing rather than a one-sided difference reported under the same name.
    #[test]
    fn a_position_at_the_edge_of_the_trace_has_no_centred_derivative() {
        let values = ramp(1000.0, 100);
        assert!(centred_derivative_at(&values, 0.0, SAMPLE_INTERVAL_SECONDS).is_none());
        assert!(centred_derivative_at(&values, 0.5, SAMPLE_INTERVAL_SECONDS).is_none());
        assert!(centred_derivative_at(&values, 99.0, SAMPLE_INTERVAL_SECONDS).is_none());
        assert!(centred_derivative_at(&values, f64::NAN, SAMPLE_INTERVAL_SECONDS).is_none());
        // The last position that has one, so the guard above is a bound rather than a wall.
        assert!(centred_derivative_at(&values, 98.0, SAMPLE_INTERVAL_SECONDS).is_some());
    }
}
