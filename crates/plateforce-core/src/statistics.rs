//! Summary statistics over a force window, and the estimator choices that are not
//! interchangeable between implementations.

/// Which divisor a standard deviation uses.
///
/// Python tools reach `numpy.std`, which divides by n. R tools reach `sd`, which
/// divides by n-1. No paper in this corpus states which it means, so the estimator is
/// carried as a bound parameter rather than picked once for the whole library.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DispersionEstimator {
    /// Divide by n. `numpy.std` default.
    Population,
    /// Divide by n-1. R `sd`, and `numpy.std(ddof=1)`.
    Sample,
}

impl DispersionEstimator {
    /// Every value this choice takes, in the word every surface spells it by.
    ///
    /// The words are here rather than at each place that needs one. Four functions in three
    /// crates spelled them for themselves, and a match that is exhaustive stops a value
    /// arriving without a word while doing nothing about two of those functions disagreeing
    /// about which word. Two vocabularies for one fact is the defect this project exists
    /// against, one layer out from two implementations of one quantity.
    pub const PUBLISHED: [&'static str; 2] = ["population", "sample"];

    /// The word this value is spelled by, indexed into the list above rather than written out,
    /// so a value added to this choice does not compile until the list names it too.
    pub fn as_published_str(self) -> &'static str {
        match self {
            DispersionEstimator::Population => Self::PUBLISHED[0],
            DispersionEstimator::Sample => Self::PUBLISHED[1],
        }
    }

    /// The value a word names, or nothing where this choice has no such word.
    pub fn from_published_str(written: &str) -> Option<Self> {
        Self::EVERY
            .into_iter()
            .find(|value| value.as_published_str() == written)
    }

    /// Every value, so a caller offering the choice walks the values rather than the words and
    /// cannot offer one that parses back to nothing.
    ///
    /// Held to `PUBLISHED` by `every_value_this_choice_takes_has_one_word_and_one_home`, which
    /// is the direction the exhaustive match above cannot ask: a word may be added to the list
    /// without a value ever taking it.
    pub const EVERY: [Self; 2] = [DispersionEstimator::Population, DispersionEstimator::Sample];

    fn degrees_of_freedom_offset(self) -> f64 {
        match self {
            DispersionEstimator::Population => 0.0,
            DispersionEstimator::Sample => 1.0,
        }
    }
}

/// Neumaier compensated summation, so a 6000 sample window does not accumulate the
/// naive error that would otherwise sit at the same order as the differences this
/// project measures.
#[derive(Debug, Clone, Copy, Default)]
pub struct CompensatedAccumulator {
    running: f64,
    compensation: f64,
}

impl CompensatedAccumulator {
    pub fn add(&mut self, value: f64) {
        let total = self.running + value;
        if self.running.abs() >= value.abs() {
            self.compensation += (self.running - total) + value;
        } else {
            self.compensation += (value - total) + self.running;
        }
        self.running = total;
    }

    pub fn total(&self) -> f64 {
        self.running + self.compensation
    }
}

pub fn compensated_sum(values: &[f64]) -> f64 {
    let mut accumulator = CompensatedAccumulator::default();
    for &value in values {
        accumulator.add(value);
    }
    accumulator.total()
}

pub fn mean(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    Some(compensated_sum(values) / values.len() as f64)
}

/// Mean and dispersion in one pass over the slice, for the rules that recompute both
/// at every sample of a trailing window.
pub fn mean_and_standard_deviation(
    values: &[f64],
    estimator: DispersionEstimator,
) -> Option<(f64, f64)> {
    let window_mean = mean(values)?;
    let divisor = values.len() as f64 - estimator.degrees_of_freedom_offset();
    if divisor <= 0.0 {
        return None;
    }
    let mut accumulator = CompensatedAccumulator::default();
    for &value in values {
        accumulator.add((value - window_mean).powi(2));
    }
    Some((window_mean, (accumulator.total() / divisor).sqrt()))
}

/// Middle order statistic, averaging the two central values on an even count.
pub fn median(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.total_cmp(b));
    let midpoint = sorted.len() / 2;
    if sorted.len() % 2 == 0 {
        Some((sorted[midpoint - 1] + sorted[midpoint]) / 2.0)
    } else {
        Some(sorted[midpoint])
    }
}

/// Two pass variance, which does not suffer the cancellation of the sum of squares
/// form when the window mean is large relative to its spread. Quiet standing on a
/// force plate is exactly that case: a mean near 600 N with a spread near 2 N.
pub fn variance(values: &[f64], estimator: DispersionEstimator) -> Option<f64> {
    standard_deviation(values, estimator).map(|deviation| deviation * deviation)
}

pub fn standard_deviation(values: &[f64], estimator: DispersionEstimator) -> Option<f64> {
    mean_and_standard_deviation(values, estimator).map(|(_, deviation)| deviation)
}

/// First index attaining the minimum. Ties go to the earliest sample, and plate output
/// is quantised enough that exact ties are common.
pub fn index_of_minimum(values: &[f64]) -> Option<usize> {
    let mut best: Option<(usize, f64)> = None;
    for (index, &value) in values.iter().enumerate() {
        if value.is_nan() {
            continue;
        }
        match best {
            Some((_, current)) if value >= current => {}
            _ => best = Some((index, value)),
        }
    }
    best.map(|(index, _)| index)
}

/// First index attaining the maximum.
pub fn index_of_maximum(values: &[f64]) -> Option<usize> {
    let mut best: Option<(usize, f64)> = None;
    for (index, &value) in values.iter().enumerate() {
        if value.is_nan() {
            continue;
        }
        match best {
            Some((_, current)) if value <= current => {}
            _ => best = Some((index, value)),
        }
    }
    best.map(|(index, _)| index)
}

/// How a rolling variance is accumulated.
///
/// The two forms are algebraically identical and select different windows on real
/// data. See `WeighingWindowSearch::tied_window_count`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VarianceAccumulation {
    /// Recompute the mean per window. Accurate, and the default for anything whose
    /// answer should not depend on the arithmetic path.
    TwoPass,
    /// Running sums of values and of squares, differenced per window. The form used by
    /// the tools this corpus reimplements, and reproducible only in the same order.
    CumulativeSumOfSquares,
}

/// The window a lowest variance rule selected, and how far the rule was from
/// determining it.
///
/// A rule that selects the minimum of a quantity with exact ties does not identify a
/// single window. On this corpus the tie runs to 417 windows, 347 ms of equally valid
/// starts, and anything downstream that treats the selection as determined is reading
/// an artefact of the arithmetic.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WeighingWindowSearch {
    pub start_index: usize,
    pub mean_newtons: f64,
    pub variance_newtons_squared: f64,
    pub candidate_window_count: usize,
    pub tied_window_count: usize,
    pub rejected_window_count: usize,
    /// Lightest and heaviest system weight the rule could have returned. The rule does
    /// not choose between them, so the difference is the width of the undeclared
    /// choice, and it is the denominator or offset for most quantities downstream.
    pub tied_weight_low_newtons: f64,
    pub tied_weight_high_newtons: f64,
}

/// Lowest variance window of a fixed length, optionally rejecting any window that
/// touches a low force floor.
///
/// Without the floor the global variance minimum is the flight phase, where the plate
/// is unloaded and almost noiseless, so the rule would weigh the athlete during the
/// part of the trial where they are in the air.
pub fn lowest_variance_window(
    values: &[f64],
    window_samples: usize,
    reject_at_or_below_newtons: Option<f64>,
    accumulation: VarianceAccumulation,
) -> Option<WeighingWindowSearch> {
    if window_samples < 2 || values.len() < window_samples {
        return None;
    }
    let window_count = values.len() - window_samples + 1;
    let admissible = admissible_windows(values, window_samples, reject_at_or_below_newtons);
    let rejected = admissible.iter().filter(|ok| !**ok).count();

    let (means, variances) = match accumulation {
        VarianceAccumulation::TwoPass => two_pass_window_statistics(values, window_samples),
        VarianceAccumulation::CumulativeSumOfSquares => {
            cumulative_window_statistics(values, window_samples)
        }
    };

    let mut best: Option<(usize, f64)> = None;
    for index in 0..window_count {
        if !admissible[index] {
            continue;
        }
        match best {
            Some((_, current)) if variances[index] >= current => {}
            _ => best = Some((index, variances[index])),
        }
    }
    let (start_index, best_variance) = best?;
    let tied: Vec<f64> = (0..window_count)
        .filter(|&index| admissible[index] && variances[index] == best_variance)
        .map(|index| means[index])
        .collect();

    Some(WeighingWindowSearch {
        start_index,
        mean_newtons: means[start_index],
        variance_newtons_squared: best_variance,
        candidate_window_count: window_count - rejected,
        tied_window_count: tied.len(),
        rejected_window_count: rejected,
        tied_weight_low_newtons: tied.iter().copied().fold(f64::INFINITY, f64::min),
        tied_weight_high_newtons: tied.iter().copied().fold(f64::NEG_INFINITY, f64::max),
    })
}

fn admissible_windows(
    values: &[f64],
    window_samples: usize,
    reject_at_or_below_newtons: Option<f64>,
) -> Vec<bool> {
    let window_count = values.len() - window_samples + 1;
    let Some(floor) = reject_at_or_below_newtons else {
        return vec![true; window_count];
    };
    let mut low_count_prefix = vec![0usize; values.len() + 1];
    for (index, &value) in values.iter().enumerate() {
        low_count_prefix[index + 1] = low_count_prefix[index] + usize::from(value <= floor);
    }
    (0..window_count)
        .map(|start| low_count_prefix[start + window_samples] == low_count_prefix[start])
        .collect()
}

fn two_pass_window_statistics(values: &[f64], window_samples: usize) -> (Vec<f64>, Vec<f64>) {
    let window_count = values.len() - window_samples + 1;
    let mut means = Vec::with_capacity(window_count);
    let mut variances = Vec::with_capacity(window_count);
    for start in 0..window_count {
        let window = &values[start..start + window_samples];
        let (window_mean, deviation) =
            mean_and_standard_deviation(window, DispersionEstimator::Population)
                .unwrap_or((f64::NAN, f64::NAN));
        means.push(window_mean);
        variances.push(deviation * deviation);
    }
    (means, variances)
}

/// Sequential running sums, differenced per window. The accumulation order is the
/// observable part of this function, not an implementation detail, because the tie
/// structure of the result depends on it.
fn cumulative_window_statistics(values: &[f64], window_samples: usize) -> (Vec<f64>, Vec<f64>) {
    let mut sum_prefix = vec![0.0f64; values.len() + 1];
    let mut square_prefix = vec![0.0f64; values.len() + 1];
    for (index, &value) in values.iter().enumerate() {
        sum_prefix[index + 1] = sum_prefix[index] + value;
        square_prefix[index + 1] = square_prefix[index] + value * value;
    }
    let window_count = values.len() - window_samples + 1;
    let width = window_samples as f64;
    let mut means = Vec::with_capacity(window_count);
    let mut variances = Vec::with_capacity(window_count);
    for start in 0..window_count {
        let sum = sum_prefix[start + window_samples] - sum_prefix[start];
        let square_sum = square_prefix[start + window_samples] - square_prefix[start];
        means.push(sum / width);
        variances.push(square_sum / width - (sum / width) * (sum / width));
    }
    (means, variances)
}

/// Sample count for a duration, under an explicit rounding policy.
///
/// The policy is a parameter because the tools differ: truncation and rounding pick
/// different sample counts wherever the product is not exact, and at 1200 Hz every
/// duration in this corpus happens to land exactly, which hides the difference.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DurationRounding {
    Truncate,
    Nearest,
    Ceiling,
}

pub fn samples_for_duration(
    duration_seconds: f64,
    sample_rate_hz: f64,
    rounding: DurationRounding,
) -> usize {
    let exact = duration_seconds * sample_rate_hz;
    if !exact.is_finite() || exact <= 0.0 {
        return 0;
    }
    let rounded = match rounding {
        DurationRounding::Truncate => exact.trunc(),
        DurationRounding::Nearest => exact.round(),
        DurationRounding::Ceiling => exact.ceil(),
    };
    rounded as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_two_dispersion_estimators_are_not_interchangeable() {
        let window = [1.0, 2.0, 3.0, 4.0];
        let population = standard_deviation(&window, DispersionEstimator::Population).unwrap();
        let sample = standard_deviation(&window, DispersionEstimator::Sample).unwrap();
        assert!(
            (population - 1.118_033_988_749_895).abs() < 1e-12,
            "{population}"
        );
        assert!((sample - 1.290_994_448_735_806).abs() < 1e-12, "{sample}");
    }

    #[test]
    fn median_averages_the_two_central_values_on_an_even_count() {
        assert_eq!(median(&[4.0, 1.0, 3.0, 2.0]), Some(2.5));
        assert_eq!(median(&[4.0, 1.0, 3.0]), Some(3.0));
    }

    #[test]
    fn compensated_summation_beats_the_naive_loop_on_a_hostile_sequence() {
        let mut values = vec![1.0e16, 1.0];
        values.extend(std::iter::repeat_n(1.0, 9));
        let naive: f64 = values.iter().sum();
        assert_eq!(naive, 1.0e16);
        assert_eq!(compensated_sum(&values), 1.0e16 + 10.0);
    }

    #[test]
    fn a_tied_variance_plateau_is_reported_rather_than_hidden() {
        // A flat segment gives every window inside it an identical variance, so the
        // rule selects a plateau and not a window.
        let mut values = vec![600.0; 40];
        values.extend((0..20).map(|i| 600.0 + i as f64));
        let found =
            lowest_variance_window(&values, 8, None, VarianceAccumulation::TwoPass).unwrap();
        assert_eq!(found.start_index, 0);
        assert_eq!(found.tied_window_count, 34);
        assert_eq!(found.tied_weight_low_newtons, 600.0);
        assert_eq!(found.tied_weight_high_newtons, 600.0);
    }

    /// A tie whose members disagree on the mean is the case that propagates: the rule
    /// returns one weight and had no grounds to prefer it over the others.
    #[test]
    fn a_tie_reports_the_span_of_weights_it_could_have_returned() {
        let mut values = vec![600.0; 12];
        values.extend(std::iter::repeat_n(605.0, 12));
        let found =
            lowest_variance_window(&values, 6, None, VarianceAccumulation::TwoPass).unwrap();
        assert_eq!(found.tied_weight_low_newtons, 600.0);
        assert_eq!(found.tied_weight_high_newtons, 605.0);
        assert!(found.tied_window_count > 1);
    }

    #[test]
    fn the_low_force_floor_keeps_the_flight_phase_out_of_the_weighing_window() {
        let mut values: Vec<f64> = (0..40).map(|i| 600.0 + (i % 3) as f64).collect();
        values.extend(std::iter::repeat_n(0.0, 40));
        let unfiltered =
            lowest_variance_window(&values, 10, None, VarianceAccumulation::TwoPass).unwrap();
        let filtered =
            lowest_variance_window(&values, 10, Some(10.0), VarianceAccumulation::TwoPass).unwrap();
        assert!(
            unfiltered.start_index >= 40,
            "unloaded plate won on variance"
        );
        assert!(
            filtered.start_index < 40,
            "floor did not exclude the flight phase"
        );
        assert_eq!(filtered.rejected_window_count, 40);
    }

    #[test]
    fn index_of_minimum_takes_the_earliest_of_a_tie() {
        assert_eq!(index_of_minimum(&[3.0, 1.0, 1.0, 2.0]), Some(1));
        assert_eq!(index_of_maximum(&[3.0, 1.0, 3.0, 2.0]), Some(0));
    }

    /// The list of words and the list of values are one enumeration, walked from either end.
    ///
    /// The match in `as_published_str` indexes `PUBLISHED`, so a value added to this choice
    /// cannot compile without a word. Nothing in the types asks the other two questions: a word
    /// added to the list that no value takes, which a caller would be offered and refused, and
    /// a value left out of `EVERY`, which is a choice the software holds and never offers.
    #[test]
    fn every_value_this_choice_takes_has_one_word_and_one_home() {
        assert_eq!(
            DispersionEstimator::EVERY.len(),
            DispersionEstimator::PUBLISHED.len(),
            "{} values are enumerated and {} words are published, so one of the two is short",
            DispersionEstimator::EVERY.len(),
            DispersionEstimator::PUBLISHED.len(),
        );
        for word in DispersionEstimator::PUBLISHED {
            let value = DispersionEstimator::from_published_str(word)
                .unwrap_or_else(|| panic!("{word} is published and no value takes it"));
            assert_eq!(value.as_published_str(), word);
        }
        for value in DispersionEstimator::EVERY {
            assert_eq!(
                DispersionEstimator::from_published_str(value.as_published_str()),
                Some(value),
                "{value:?} spells itself as a word that parses back to something else"
            );
        }
        assert_eq!(DispersionEstimator::from_published_str("unbiased"), None);
    }
}
