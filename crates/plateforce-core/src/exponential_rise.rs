//! A mono-exponential rise fitted to a stretch of the force trace by least squares.
//!
//! Two registry entries read one fit: the asymptote it approaches is a published peak-force
//! convention, and the rates read off it are a published rate variant. Fitting twice would be
//! two answers to one question, so the fit is here and both callers read the same object.
//!
//! The model is `F(t) = baseline + amplitude x (1 - exp(-t / time_constant))`, with `t`
//! measured from the first sample of the fitted stretch. It is linear in the baseline and the
//! amplitude and non-linear in the time constant alone, so the search runs over the time
//! constant and solves the other two exactly at each candidate. That is deterministic and has
//! no starting guess to get wrong, which a general non-linear solver would.
//!
//! Real isometric force rise is sigmoidal, with a low-rate foot before the steep phase, while
//! a mono-exponential pinned at the start has its maximum slope at time zero. So the model
//! overestimates early rate and underestimates it later, and the size of that is what the mean
//! absolute residual measures. Every number this returns travels with that residual.

/// Why no rise could be fitted.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FitError {
    /// Three samples is the fewest that can constrain three parameters.
    #[error("a rise was fitted over samples {start} to {end}, which holds fewer than three")]
    SpanTooShort { start: usize, end: usize },
    /// The stretch carries no variation, or a value that is not a number, so no rise is
    /// separable from a flat line.
    #[error("the stretch carries no rise to fit")]
    NothingToFit,
}

/// A fitted mono-exponential rise, and how far it sits from the samples it was fitted to.
///
/// The residual travels in the struct rather than beside it, because a fitted number without
/// it cannot be told from a measured one, and the source that ships this rule prints the
/// residual in red above 5 percent for exactly that reason.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ExponentialRise {
    pub baseline_newtons: f64,
    pub amplitude_newtons: f64,
    pub time_constant_seconds: f64,
    /// The mean of `|measured - fitted| / |measured|` over the fitted stretch, as a
    /// percentage.
    pub mean_absolute_residual_percent: f64,
}

impl ExponentialRise {
    /// The value the model approaches as time goes to infinity, which is never attained in a
    /// trial. One published peak-force convention reports this number.
    pub fn asymptote_newtons(&self) -> f64 {
        self.baseline_newtons + self.amplitude_newtons
    }

    /// The modelled force at a stated time from the start of the fitted stretch.
    pub fn force_at_seconds(&self, seconds: f64) -> f64 {
        self.baseline_newtons
            + self.amplitude_newtons * (1.0 - (-seconds / self.time_constant_seconds).exp())
    }

    /// The modelled instantaneous rate at a stated time from the start of the fitted stretch.
    pub fn rate_at_seconds(&self, seconds: f64) -> f64 {
        self.amplitude_newtons / self.time_constant_seconds
            * (-seconds / self.time_constant_seconds).exp()
    }

    /// The largest rate the model reaches, which is its rate at time zero.
    ///
    /// A property of the model rather than a search over it: the exponential's slope decays
    /// monotonically from the start of the fitted stretch, so its maximum is the first
    /// instant. This is the one reading of the fit that needs no parameter the entry does not
    /// publish.
    pub fn maximum_rate_newtons_per_second(&self) -> f64 {
        self.rate_at_seconds(0.0)
    }
}

/// The golden ratio's reciprocal, which fixes where the search places its two probes.
const GOLDEN_SECTION: f64 = 0.618_033_988_749_895;

/// Iterations of the search over the time constant.
///
/// The interval shrinks by the golden ratio each time, so 200 takes an interval spanning four
/// decades of time constant to below one part in 10^40, far under the precision of an f64. It
/// is a fixed count rather than a tolerance so that two runs on one recording take the same
/// path and return the same bytes.
const SEARCH_ITERATIONS: usize = 200;

/// The best mono-exponential rise over the half-open span `[start, end)`.
///
/// The search runs over the logarithm of the time constant, because a time constant is a
/// scale: the interesting range spans from one sample interval to the length of the stretch,
/// and a linear search over that spends almost all of its probes on the slow end.
pub fn fit_exponential_rise(
    values: &[f64],
    start: usize,
    end: usize,
    sample_interval_seconds: f64,
) -> Result<ExponentialRise, FitError> {
    let end = end.min(values.len());
    if start >= end || end - start < 3 {
        return Err(FitError::SpanTooShort { start, end });
    }
    let observed = &values[start..end];
    if observed.iter().any(|value| !value.is_finite()) {
        return Err(FitError::NothingToFit);
    }
    let span_seconds = (observed.len() - 1) as f64 * sample_interval_seconds;
    if span_seconds <= 0.0 || sample_interval_seconds <= 0.0 {
        return Err(FitError::NothingToFit);
    }

    // One sample interval to the whole stretch. A time constant below the first is faster
    // than the recording can see, and one above the second is a rise the stretch does not
    // contain enough of to distinguish from a straight line.
    let mut low = sample_interval_seconds.ln();
    let mut high = span_seconds.ln();
    let mut left = high - GOLDEN_SECTION * (high - low);
    let mut right = low + GOLDEN_SECTION * (high - low);
    let mut left_error = sum_of_squares(observed, left.exp(), sample_interval_seconds);
    let mut right_error = sum_of_squares(observed, right.exp(), sample_interval_seconds);

    for _ in 0..SEARCH_ITERATIONS {
        if left_error <= right_error {
            high = right;
            right = left;
            right_error = left_error;
            left = high - GOLDEN_SECTION * (high - low);
            left_error = sum_of_squares(observed, left.exp(), sample_interval_seconds);
        } else {
            low = left;
            left = right;
            left_error = right_error;
            right = low + GOLDEN_SECTION * (high - low);
            right_error = sum_of_squares(observed, right.exp(), sample_interval_seconds);
        }
    }

    let time_constant_seconds = ((low + high) / 2.0).exp();
    let Some((baseline_newtons, amplitude_newtons)) =
        solve_at(observed, time_constant_seconds, sample_interval_seconds)
    else {
        return Err(FitError::NothingToFit);
    };
    if !baseline_newtons.is_finite() || !amplitude_newtons.is_finite() {
        return Err(FitError::NothingToFit);
    }

    let fitted = ExponentialRise {
        baseline_newtons,
        amplitude_newtons,
        time_constant_seconds,
        mean_absolute_residual_percent: 0.0,
    };
    let residual = mean_absolute_residual_percent(observed, &fitted, sample_interval_seconds)
        .ok_or(FitError::NothingToFit)?;
    Ok(ExponentialRise {
        mean_absolute_residual_percent: residual,
        ..fitted
    })
}

/// The baseline and amplitude that minimise the squared error at one time constant.
///
/// With the time constant held, the model is a straight line through two known columns, so
/// the answer is the two-by-two normal equations rather than another search. `None` where the
/// columns are parallel, which is a stretch carrying no rise to separate from its level.
fn solve_at(
    observed: &[f64],
    time_constant_seconds: f64,
    sample_interval_seconds: f64,
) -> Option<(f64, f64)> {
    let count = observed.len() as f64;
    let mut sum_basis = 0.0;
    let mut sum_basis_squared = 0.0;
    let mut sum_observed = 0.0;
    let mut sum_basis_observed = 0.0;
    for (index, value) in observed.iter().enumerate() {
        let seconds = index as f64 * sample_interval_seconds;
        let basis = 1.0 - (-seconds / time_constant_seconds).exp();
        sum_basis += basis;
        sum_basis_squared += basis * basis;
        sum_observed += value;
        sum_basis_observed += basis * value;
    }
    let determinant = count * sum_basis_squared - sum_basis * sum_basis;
    if determinant.abs() < f64::EPSILON {
        return None;
    }
    let baseline =
        (sum_basis_squared * sum_observed - sum_basis * sum_basis_observed) / determinant;
    let amplitude = (count * sum_basis_observed - sum_basis * sum_observed) / determinant;
    Some((baseline, amplitude))
}

/// The squared error at one time constant, which is what the search over it minimises.
fn sum_of_squares(
    observed: &[f64],
    time_constant_seconds: f64,
    sample_interval_seconds: f64,
) -> f64 {
    let Some((baseline, amplitude)) =
        solve_at(observed, time_constant_seconds, sample_interval_seconds)
    else {
        return f64::INFINITY;
    };
    let mut total = 0.0;
    for (index, value) in observed.iter().enumerate() {
        let seconds = index as f64 * sample_interval_seconds;
        let fitted = baseline + amplitude * (1.0 - (-seconds / time_constant_seconds).exp());
        total += (fitted - value) * (fitted - value);
    }
    total
}

/// The mean absolute percentage residual, over the samples that carry a force to take a
/// percentage of.
///
/// A sample reading zero has no percentage, and a stretch of them is a plate the athlete has
/// left. `None` where no sample carries one, which is a stretch this residual cannot describe.
fn mean_absolute_residual_percent(
    observed: &[f64],
    fitted: &ExponentialRise,
    sample_interval_seconds: f64,
) -> Option<f64> {
    let mut total = 0.0;
    let mut counted = 0usize;
    for (index, value) in observed.iter().enumerate() {
        if value.abs() < f64::EPSILON {
            continue;
        }
        let seconds = index as f64 * sample_interval_seconds;
        total += (fitted.force_at_seconds(seconds) - value).abs() / value.abs();
        counted += 1;
    }
    (counted > 0).then(|| total / counted as f64 * 100.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_INTERVAL_SECONDS: f64 = 1.0 / 1200.0;

    /// A rise with known parameters, so the fit is held to the numbers that generated it
    /// rather than to its own arithmetic.
    fn a_rise(baseline: f64, amplitude: f64, time_constant: f64, samples: usize) -> Vec<f64> {
        (0..samples)
            .map(|index| {
                let seconds = index as f64 * SAMPLE_INTERVAL_SECONDS;
                baseline + amplitude * (1.0 - (-seconds / time_constant).exp())
            })
            .collect()
    }

    #[test]
    fn a_noiseless_rise_returns_the_parameters_that_generated_it() {
        let values = a_rise(200.0, 3000.0, 0.15, 2400);
        let fitted = fit_exponential_rise(&values, 0, values.len(), SAMPLE_INTERVAL_SECONDS)
            .expect("the rise fits");
        assert!(
            (fitted.time_constant_seconds - 0.15).abs() < 1e-6,
            "{fitted:?}"
        );
        assert!((fitted.baseline_newtons - 200.0).abs() < 1e-6, "{fitted:?}");
        assert!(
            (fitted.amplitude_newtons - 3000.0).abs() < 1e-6,
            "{fitted:?}"
        );
        assert!(
            (fitted.asymptote_newtons() - 3200.0).abs() < 1e-6,
            "{fitted:?}"
        );
        assert!(
            fitted.mean_absolute_residual_percent < 1e-6,
            "a noiseless rise leaves a residual of {} percent",
            fitted.mean_absolute_residual_percent
        );
    }

    /// The asymptote is above every sample of the trace it was fitted to, which is the whole
    /// of what separates this peak-force convention from reading the peak off the trace.
    #[test]
    fn the_asymptote_stands_above_the_largest_sample_of_the_rise() {
        let values = a_rise(200.0, 3000.0, 0.4, 1200);
        let fitted = fit_exponential_rise(&values, 0, values.len(), SAMPLE_INTERVAL_SECONDS)
            .expect("the rise fits");
        let largest = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        assert!(
            fitted.asymptote_newtons() > largest,
            "the asymptote {} is not above the largest sample {largest}",
            fitted.asymptote_newtons()
        );
        // A trial that ran further into the plateau leaves a smaller gap, which is the
        // sensitivity the registry records for this convention.
        let longer = a_rise(200.0, 3000.0, 0.4, 4800);
        let further = fit_exponential_rise(&longer, 0, longer.len(), SAMPLE_INTERVAL_SECONDS)
            .expect("the rise fits");
        let further_largest = longer.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        assert!(
            further.asymptote_newtons() - further_largest < fitted.asymptote_newtons() - largest,
            "a longer trial did not close the gap between the asymptote and the trace"
        );
    }

    /// The maximum modelled rate is the amplitude over the time constant, and it is reached at
    /// the start of the fitted stretch rather than found by searching.
    #[test]
    fn the_maximum_modelled_rate_is_the_amplitude_over_the_time_constant() {
        let values = a_rise(0.0, 2000.0, 0.25, 1200);
        let fitted = fit_exponential_rise(&values, 0, values.len(), SAMPLE_INTERVAL_SECONDS)
            .expect("the rise fits");
        assert!(
            (fitted.maximum_rate_newtons_per_second() - 8000.0).abs() < 1e-3,
            "{fitted:?}"
        );
        for seconds in [0.01, 0.1, 0.5] {
            assert!(
                fitted.rate_at_seconds(seconds) < fitted.maximum_rate_newtons_per_second(),
                "the rate at {seconds} s is not below the maximum"
            );
        }
    }

    /// A trace that is not an exponential rise still fits, and says how far it is from one.
    /// The residual is the number that carries that, so it has to move with the shape.
    #[test]
    fn the_residual_reports_how_far_the_trace_is_from_the_model() {
        let exponential = a_rise(200.0, 3000.0, 0.15, 1200);
        let close =
            fit_exponential_rise(&exponential, 0, exponential.len(), SAMPLE_INTERVAL_SECONDS)
                .expect("the rise fits");

        // A straight ramp over the same range, which no exponential passes through.
        let ramp: Vec<f64> = (0..1200)
            .map(|index| 200.0 + 3000.0 * index as f64 / 1199.0)
            .collect();
        let far = fit_exponential_rise(&ramp, 0, ramp.len(), SAMPLE_INTERVAL_SECONDS)
            .expect("the ramp fits");
        println!(
            "exponential {:.6} percent, ramp {:.6} percent",
            close.mean_absolute_residual_percent, far.mean_absolute_residual_percent
        );
        assert!(
            far.mean_absolute_residual_percent > close.mean_absolute_residual_percent,
            "the ramp leaves no larger residual than the exponential it is not"
        );
    }

    #[test]
    fn a_stretch_too_short_or_flat_to_carry_a_rise_is_refused() {
        let values = a_rise(200.0, 3000.0, 0.15, 1200);
        assert_eq!(
            fit_exponential_rise(&values, 0, 2, SAMPLE_INTERVAL_SECONDS).unwrap_err(),
            FitError::SpanTooShort { start: 0, end: 2 }
        );
        let carrying_nothing = vec![f64::NAN; 1200];
        assert_eq!(
            fit_exponential_rise(&carrying_nothing, 0, 1200, SAMPLE_INTERVAL_SECONDS).unwrap_err(),
            FitError::NothingToFit
        );
    }
}
