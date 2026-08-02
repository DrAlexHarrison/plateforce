//! Choosing a filter cutoff by a published selection rule, and reporting what chose it.
//!
//! Both rules here score a candidate cutoff by what the filter removed. Neither was
//! validated on ground reaction force, and around takeoff and touchdown the transition
//! between full system weight and zero is a genuine step. A step is broadband, so no
//! cutoff separates signal from noise there and a rule that returns a number anyway has
//! answered a question it was not asked. The interval a caller wants a cutoff for is
//! therefore an argument, and a rule declines the interval rather than the method.
//!
//! One tool computes the whiteness statistic, prints it to a console and surfaces it
//! nowhere. The statistic travels with the cutoff here, because a cutoff nobody can trace
//! to the number that chose it is the unrecorded choice this software exists to end.

use crate::butterworth::{low_pass_dual_pass_zero_lag, ButterworthError, StateInitialisation};
use crate::statistics::compensated_sum;

/// A straight line fitted by ordinary least squares.
///
/// Distinct from the ordinary-least-products regression in `agreement`, which minimises
/// the product of both residuals and answers a different question: this one treats the
/// abscissa as known, which is what a cutoff grid is.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LeastSquaresLine {
    pub slope: f64,
    pub intercept: f64,
}

impl LeastSquaresLine {
    pub fn at(&self, position: f64) -> f64 {
        self.intercept + self.slope * position
    }
}

/// Least squares line through paired points, or nothing when the abscissa does not vary.
pub fn least_squares_line(abscissa: &[f64], ordinate: &[f64]) -> Option<LeastSquaresLine> {
    if abscissa.len() != ordinate.len() || abscissa.len() < 2 {
        return None;
    }
    let count = abscissa.len() as f64;
    let mean_abscissa = compensated_sum(abscissa) / count;
    let mean_ordinate = compensated_sum(ordinate) / count;
    let products: Vec<f64> = abscissa
        .iter()
        .zip(ordinate)
        .map(|(x, y)| (x - mean_abscissa) * (y - mean_ordinate))
        .collect();
    let squares: Vec<f64> = abscissa
        .iter()
        .map(|x| (x - mean_abscissa) * (x - mean_abscissa))
        .collect();
    let denominator = compensated_sum(&squares);
    if denominator == 0.0 || !denominator.is_finite() {
        return None;
    }
    let slope = compensated_sum(&products) / denominator;
    Some(LeastSquaresLine {
        slope,
        intercept: mean_ordinate - slope * mean_abscissa,
    })
}

/// Durbin-Watson statistic of a residual series.
///
/// Two for a series with no lag-one autocorrelation, toward zero for positive
/// autocorrelation and toward four for negative.
pub fn durbin_watson(residuals: &[f64]) -> Option<f64> {
    if residuals.len() < 2 {
        return None;
    }
    let differences: Vec<f64> = residuals
        .windows(2)
        .map(|pair| (pair[1] - pair[0]) * (pair[1] - pair[0]))
        .collect();
    let squares: Vec<f64> = residuals.iter().map(|value| value * value).collect();
    let denominator = compensated_sum(&squares);
    if denominator == 0.0 || !denominator.is_finite() {
        return None;
    }
    Some(compensated_sum(&differences) / denominator)
}

/// How far a residual series is from white, on the scale the source states.
///
/// Zero is white. The transform is stated in the entry rather than derived here, so it is
/// reproduced rather than improved.
pub fn whiteness_score(durbin_watson_statistic: f64) -> f64 {
    (2.0 - durbin_watson_statistic).abs() / 2.0
}

/// Root mean square of what a filter removed.
pub fn residual_root_mean_square(raw: &[f64], filtered: &[f64]) -> Option<f64> {
    if raw.len() != filtered.len() || raw.is_empty() {
        return None;
    }
    let squares: Vec<f64> = raw
        .iter()
        .zip(filtered)
        .map(|(before, after)| (before - after) * (before - after))
        .collect();
    Some((compensated_sum(&squares) / raw.len() as f64).sqrt())
}

/// A chosen cutoff and the number that chose it.
#[derive(Debug, Clone, PartialEq)]
pub struct CutoffSelection {
    pub cutoff_hz: f64,
    /// What the rule scored this cutoff on: the residual for Winter, the whiteness score
    /// for Challis. Carried so the choice can be audited rather than taken on trust.
    pub score: f64,
    /// Present only for Winter, where the intercept is the noise level the rule matched.
    pub noise_intercept: Option<f64>,
    /// Present only for Challis, whose score is a transform of this.
    pub durbin_watson_statistic: Option<f64>,
}

/// Either a cutoff, or the interval over which the rule is out of its validity domain.
#[derive(Debug, Clone, PartialEq)]
pub enum CutoffOutcome {
    Selected(CutoffSelection),
    /// The interval spans a step in the trace, where no cutoff separates signal from
    /// noise. The indices are the step the interval contains.
    IntervalSpansAStep {
        first_step_index: usize,
    },
    /// Every candidate was scored and none could be, which is a different state from a
    /// rule that was never allowed to run.
    NoCandidateScored,
}

fn step_inside(interval: (usize, usize), step_indices: &[usize]) -> Option<usize> {
    step_indices
        .iter()
        .copied()
        .filter(|index| *index >= interval.0 && *index < interval.1)
        .min()
}

fn residual_curve(
    values: &[f64],
    interval: (usize, usize),
    candidate_cutoffs_hz: &[f64],
    order: usize,
    sample_rate_hz: f64,
    initialisation: StateInitialisation,
) -> Result<Vec<(f64, f64, Vec<f64>)>, ButterworthError> {
    let segment = &values[interval.0..interval.1];
    let mut curve = Vec::with_capacity(candidate_cutoffs_hz.len());
    for &cutoff_hz in candidate_cutoffs_hz {
        let filtered =
            low_pass_dual_pass_zero_lag(segment, cutoff_hz, order, sample_rate_hz, initialisation)?;
        let Some(residual) = residual_root_mean_square(segment, &filtered) else {
            continue;
        };
        let removed: Vec<f64> = segment
            .iter()
            .zip(&filtered)
            .map(|(before, after)| before - after)
            .collect();
        curve.push((cutoff_hz, residual, removed));
    }
    Ok(curve)
}

/// Winter's residual analysis: extrapolate the noise-dominated tail back to zero cutoff,
/// and take the cutoff whose residual matches that intercept.
///
/// `noise_dominated_from_hz` is where the caller says the curve has become linear in
/// noise. No published source states it, so it is asked for rather than assumed.
#[allow(clippy::too_many_arguments)]
pub fn select_cutoff_by_residual_analysis(
    values: &[f64],
    interval: (usize, usize),
    step_indices: &[usize],
    candidate_cutoffs_hz: &[f64],
    order: usize,
    sample_rate_hz: f64,
    initialisation: StateInitialisation,
    noise_dominated_from_hz: f64,
) -> Result<CutoffOutcome, ButterworthError> {
    if let Some(first_step_index) = step_inside(interval, step_indices) {
        return Ok(CutoffOutcome::IntervalSpansAStep { first_step_index });
    }
    let curve = residual_curve(
        values,
        interval,
        candidate_cutoffs_hz,
        order,
        sample_rate_hz,
        initialisation,
    )?;
    let tail: Vec<(f64, f64)> = curve
        .iter()
        .filter(|(cutoff_hz, _, _)| *cutoff_hz >= noise_dominated_from_hz)
        .map(|(cutoff_hz, residual, _)| (*cutoff_hz, *residual))
        .collect();
    let abscissa: Vec<f64> = tail.iter().map(|(cutoff, _)| *cutoff).collect();
    let ordinate: Vec<f64> = tail.iter().map(|(_, residual)| *residual).collect();
    let Some(line) = least_squares_line(&abscissa, &ordinate) else {
        return Ok(CutoffOutcome::NoCandidateScored);
    };
    let noise_intercept = line.at(0.0);
    let chosen = curve
        .iter()
        .min_by(|left, right| {
            (left.1 - noise_intercept)
                .abs()
                .total_cmp(&(right.1 - noise_intercept).abs())
        })
        .map(|(cutoff_hz, residual, _)| CutoffSelection {
            cutoff_hz: *cutoff_hz,
            score: *residual,
            noise_intercept: Some(noise_intercept),
            durbin_watson_statistic: None,
        });
    Ok(chosen.map_or(CutoffOutcome::NoCandidateScored, CutoffOutcome::Selected))
}

/// Challis's whiteness rule: the cutoff whose removed component is closest to white.
#[allow(clippy::too_many_arguments)]
pub fn select_cutoff_by_autocorrelation_whiteness(
    values: &[f64],
    interval: (usize, usize),
    step_indices: &[usize],
    candidate_cutoffs_hz: &[f64],
    order: usize,
    sample_rate_hz: f64,
    initialisation: StateInitialisation,
) -> Result<CutoffOutcome, ButterworthError> {
    if let Some(first_step_index) = step_inside(interval, step_indices) {
        return Ok(CutoffOutcome::IntervalSpansAStep { first_step_index });
    }
    let curve = residual_curve(
        values,
        interval,
        candidate_cutoffs_hz,
        order,
        sample_rate_hz,
        initialisation,
    )?;
    let chosen = curve
        .iter()
        .filter_map(|(cutoff_hz, _, removed)| {
            durbin_watson(removed).map(|statistic| (*cutoff_hz, statistic))
        })
        .min_by(|left, right| whiteness_score(left.1).total_cmp(&whiteness_score(right.1)))
        .map(|(cutoff_hz, statistic)| CutoffSelection {
            cutoff_hz,
            score: whiteness_score(statistic),
            noise_intercept: None,
            durbin_watson_statistic: Some(statistic),
        });
    Ok(chosen.map_or(CutoffOutcome::NoCandidateScored, CutoffOutcome::Selected))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_RATE_HZ: f64 = 1200.0;
    const CANDIDATES: [f64; 8] = [5.0, 10.0, 20.0, 40.0, 60.0, 80.0, 120.0, 160.0];

    fn deterministic_noise(index: usize) -> f64 {
        let phase = index as f64;
        3.0 * (phase * 2.399_963_2).sin() + 2.0 * (phase * 1.107_148_7).cos()
    }

    /// A slow ramp with high frequency noise on top, so the noise-dominated tail of the
    /// residual curve is real rather than an artefact of the test's own construction.
    fn slow_signal_with_noise(sample_count: usize) -> Vec<f64> {
        (0..sample_count)
            .map(|index| {
                let seconds = index as f64 / SAMPLE_RATE_HZ;
                700.0
                    + 200.0 * (2.0 * std::f64::consts::PI * 1.5 * seconds).sin()
                    + deterministic_noise(index)
            })
            .collect()
    }

    #[test]
    fn a_straight_line_fits_itself_and_reports_its_own_intercept() {
        let abscissa: Vec<f64> = (0..20).map(|index| index as f64).collect();
        let ordinate: Vec<f64> = abscissa.iter().map(|x| 4.0 + 2.5 * x).collect();
        let line = least_squares_line(&abscissa, &ordinate).unwrap();
        assert!((line.slope - 2.5).abs() < 1e-12, "{line:?}");
        assert!((line.intercept - 4.0).abs() < 1e-12, "{line:?}");
    }

    #[test]
    fn an_abscissa_that_does_not_vary_has_no_line_rather_than_an_infinite_slope() {
        assert!(least_squares_line(&[3.0, 3.0, 3.0], &[1.0, 2.0, 3.0]).is_none());
    }

    /// An alternating series is maximally negatively autocorrelated, which is the far end
    /// of the statistic's range, and a constant series has no variance to score.
    #[test]
    fn the_durbin_watson_statistic_spans_its_stated_range() {
        let alternating: Vec<f64> = (0..200)
            .map(|index| if index % 2 == 0 { 1.0 } else { -1.0 })
            .collect();
        let statistic = durbin_watson(&alternating).unwrap();
        assert!(statistic > 3.9, "{statistic}");
        assert!(whiteness_score(statistic) > 0.9);

        let drifting: Vec<f64> = (0..200).map(|index| index as f64).collect();
        let drifting_statistic = durbin_watson(&drifting).unwrap();
        assert!(drifting_statistic < 0.1, "{drifting_statistic}");

        // The statistic divides by the sum of squared residuals rather than by their
        // variance, so a constant series scores zero and only an all-zero one has no
        // denominator at all.
        assert_eq!(durbin_watson(&[5.0; 50]), Some(0.0));
        assert!(durbin_watson(&[0.0; 50]).is_none());
        assert!((whiteness_score(2.0) - 0.0).abs() < 1e-15);
    }

    #[test]
    fn a_perfectly_reproduced_signal_leaves_no_residual() {
        let values = vec![600.0; 100];
        assert_eq!(residual_root_mean_square(&values, &values), Some(0.0));
    }

    #[test]
    fn winters_rule_returns_a_cutoff_and_the_intercept_that_chose_it() {
        let values = slow_signal_with_noise(2400);
        let outcome = select_cutoff_by_residual_analysis(
            &values,
            (0, 2400),
            &[],
            &CANDIDATES,
            2,
            SAMPLE_RATE_HZ,
            StateInitialisation::Zeros,
            60.0,
        )
        .unwrap();
        let CutoffOutcome::Selected(selection) = outcome else {
            panic!("the rule declined an interval with no step in it: {outcome:?}");
        };
        assert!(CANDIDATES.contains(&selection.cutoff_hz), "{selection:?}");
        assert!(selection.noise_intercept.is_some(), "{selection:?}");
        assert!(selection.durbin_watson_statistic.is_none(), "{selection:?}");
    }

    #[test]
    fn challiss_rule_carries_the_statistic_that_chose_the_cutoff() {
        let values = slow_signal_with_noise(2400);
        let outcome = select_cutoff_by_autocorrelation_whiteness(
            &values,
            (0, 2400),
            &[],
            &CANDIDATES,
            2,
            SAMPLE_RATE_HZ,
            StateInitialisation::Zeros,
        )
        .unwrap();
        let CutoffOutcome::Selected(selection) = outcome else {
            panic!("the rule declined an interval with no step in it: {outcome:?}");
        };
        let statistic = selection
            .durbin_watson_statistic
            .expect("the statistic travels with the cutoff");
        assert!((selection.score - whiteness_score(statistic)).abs() < 1e-15);
    }

    /// The refusal is scoped to the interval and not to the method: the same trace, the
    /// same candidates, declined only where the step is.
    #[test]
    fn an_interval_containing_a_step_is_declined_and_the_same_trace_away_from_it_is_not() {
        let mut values = slow_signal_with_noise(3600);
        for value in values.iter_mut().skip(2400) {
            *value = 0.0;
        }
        let takeoff = 2400;

        let across = select_cutoff_by_residual_analysis(
            &values,
            (1200, 3000),
            &[takeoff],
            &CANDIDATES,
            2,
            SAMPLE_RATE_HZ,
            StateInitialisation::Zeros,
            60.0,
        )
        .unwrap();
        assert_eq!(
            across,
            CutoffOutcome::IntervalSpansAStep {
                first_step_index: takeoff
            }
        );

        let before = select_cutoff_by_residual_analysis(
            &values,
            (0, 2000),
            &[takeoff],
            &CANDIDATES,
            2,
            SAMPLE_RATE_HZ,
            StateInitialisation::Zeros,
            60.0,
        )
        .unwrap();
        assert!(
            matches!(before, CutoffOutcome::Selected(_)),
            "the rule declined an interval that ends before the step: {before:?}"
        );

        let whiteness_across = select_cutoff_by_autocorrelation_whiteness(
            &values,
            (1200, 3000),
            &[takeoff],
            &CANDIDATES,
            2,
            SAMPLE_RATE_HZ,
            StateInitialisation::Zeros,
        )
        .unwrap();
        assert_eq!(
            whiteness_across,
            CutoffOutcome::IntervalSpansAStep {
                first_step_index: takeoff
            }
        );
    }
}
