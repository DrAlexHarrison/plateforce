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
    let tied = (0..window_count)
        .filter(|&i| admissible[i] && variances[i] == best_variance)
        .count();

    Some(WeighingWindowSearch {
        start_index,
        mean_newtons: means[start_index],
        variance_newtons_squared: best_variance,
        candidate_window_count: window_count - rejected,
        tied_window_count: tied,
        rejected_window_count: rejected,
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
        assert!((population - 1.118_033_988_749_895).abs() < 1e-12, "{population}");
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
        values.extend(std::iter::repeat(1.0).take(9));
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
    }

    #[test]
    fn the_low_force_floor_keeps_the_flight_phase_out_of_the_weighing_window() {
        let mut values: Vec<f64> = (0..40).map(|i| 600.0 + (i % 3) as f64).collect();
        values.extend(std::iter::repeat(0.0).take(40));
        let unfiltered =
            lowest_variance_window(&values, 10, None, VarianceAccumulation::TwoPass).unwrap();
        let filtered =
            lowest_variance_window(&values, 10, Some(10.0), VarianceAccumulation::TwoPass).unwrap();
        assert!(unfiltered.start_index >= 40, "unloaded plate won on variance");
        assert!(filtered.start_index < 40, "floor did not exclude the flight phase");
        assert_eq!(filtered.rejected_window_count, 40);
    }

    #[test]
    fn index_of_minimum_takes_the_earliest_of_a_tie() {
        assert_eq!(index_of_minimum(&[3.0, 1.0, 1.0, 2.0]), Some(1));
        assert_eq!(index_of_maximum(&[3.0, 1.0, 3.0, 2.0]), Some(0));
    }
}
