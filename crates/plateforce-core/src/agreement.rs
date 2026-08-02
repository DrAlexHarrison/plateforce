//! How far two methods disagree, and how far one method wanders between trials.
//!
//! Every quantity here is computed once and nowhere else. Each function takes an explicit
//! dispersion estimator rather than assuming one, because the registry entry that publishes
//! limits of agreement carries that parameter as required with no default, and two figures
//! computed under different conventions are two different numbers reported under one name.

use crate::statistics::{mean, standard_deviation};
use crate::DispersionEstimator;

/// The multiplier on the standard deviation of the differences, from the published rule.
const NORMAL_INTERVAL_MULTIPLIER: f64 = 1.96;

/// Paired values from one repetition measured two ways.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pair {
    pub first: f64,
    pub second: f64,
}

/// Bias and the interval around it, over the pairs it was taken on.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LimitsOfAgreement {
    pub bias: f64,
    pub standard_deviation_of_differences: f64,
    pub lower: f64,
    pub upper: f64,
    /// The count the interval rests on, which travels with it everywhere.
    pub n: usize,
}

/// Mean difference and the limits around it.
///
/// Returns `None` on fewer than two pairs, where a dispersion has no meaning rather than a
/// value of zero.
pub fn limits_of_agreement(
    pairs: &[Pair],
    dispersion: DispersionEstimator,
) -> Option<LimitsOfAgreement> {
    if pairs.len() < 2 {
        return None;
    }
    let differences: Vec<f64> = pairs.iter().map(|pair| pair.first - pair.second).collect();
    let bias = mean(&differences)?;
    let spread = standard_deviation(&differences, dispersion)?;
    Some(LimitsOfAgreement {
        bias,
        standard_deviation_of_differences: spread,
        lower: bias - NORMAL_INTERVAL_MULTIPLIER * spread,
        upper: bias + NORMAL_INTERVAL_MULTIPLIER * spread,
        n: pairs.len(),
    })
}

/// Pearson product-moment correlation over the pairs.
pub fn correlation(pairs: &[Pair]) -> Option<f64> {
    if pairs.len() < 2 {
        return None;
    }
    let first: Vec<f64> = pairs.iter().map(|pair| pair.first).collect();
    let second: Vec<f64> = pairs.iter().map(|pair| pair.second).collect();
    let mean_first = mean(&first)?;
    let mean_second = mean(&second)?;

    let mut covariance = 0.0;
    let mut first_spread = 0.0;
    let mut second_spread = 0.0;
    for pair in pairs {
        let left = pair.first - mean_first;
        let right = pair.second - mean_second;
        covariance += left * right;
        first_spread += left * left;
        second_spread += right * right;
    }
    let denominator = (first_spread * second_spread).sqrt();
    if denominator == 0.0 {
        return None;
    }
    Some(covariance / denominator)
}

/// Slope and intercept of an ordinary least products regression.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ProductRegression {
    pub slope: f64,
    pub intercept: f64,
    pub n: usize,
}

/// Ordinary least products, which minimises the area rather than the vertical residual, so
/// neither method is treated as the one without error.
pub fn ordinary_least_products(
    pairs: &[Pair],
    dispersion: DispersionEstimator,
) -> Option<ProductRegression> {
    if pairs.len() < 2 {
        return None;
    }
    let first: Vec<f64> = pairs.iter().map(|pair| pair.first).collect();
    let second: Vec<f64> = pairs.iter().map(|pair| pair.second).collect();
    let slope_magnitude =
        standard_deviation(&second, dispersion)? / standard_deviation(&first, dispersion)?;
    // The sign comes from the correlation, so a negative relation is not reported as positive.
    let slope = slope_magnitude * correlation(pairs)?.signum();
    Some(ProductRegression {
        slope,
        intercept: mean(&second)? - slope * mean(&first)?,
        n: pairs.len(),
    })
}

/// A coefficient of variation, and the convention that produced it.
///
/// Two figures under different conventions are not comparable and the convention travels so
/// a caller can refuse to compare them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CoefficientOfVariation {
    pub percent: f64,
    pub dispersion: DispersionEstimator,
    pub n: usize,
}

/// Standard deviation over mean, as a percentage, for one subject's repeated trials.
pub fn coefficient_of_variation(
    values: &[f64],
    dispersion: DispersionEstimator,
) -> Option<CoefficientOfVariation> {
    if values.len() < 2 {
        return None;
    }
    let centre = mean(values)?;
    if centre == 0.0 {
        return None;
    }
    Some(CoefficientOfVariation {
        percent: standard_deviation(values, dispersion)? / centre * 100.0,
        dispersion,
        n: values.len(),
    })
}

/// The rule the registry publishes: per subject, standard deviation over mean, then averaged
/// across subjects. The average is over subjects, so the count that travels is the subject
/// count and not the trial count.
pub fn mean_coefficient_of_variation(
    per_subject: &[Vec<f64>],
    dispersion: DispersionEstimator,
) -> Option<CoefficientOfVariation> {
    let each: Vec<f64> = per_subject
        .iter()
        .filter_map(|values| coefficient_of_variation(values, dispersion))
        .map(|figure| figure.percent)
        .collect();
    if each.is_empty() {
        return None;
    }
    Some(CoefficientOfVariation {
        percent: mean(&each)?,
        dispersion,
        n: each.len(),
    })
}

/// Which of the two intraclass forms produced a figure. They are different numbers and the
/// literature reports both under one name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntraclassForm {
    /// Two-way random, absolute agreement, single measurement.
    AbsoluteAgreementSingle,
    /// Two-way mixed, consistency, single measurement.
    ConsistencySingle,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IntraclassCorrelation {
    pub value: f64,
    pub form: IntraclassForm,
    /// Subjects, which is the unit the figure is taken over.
    pub subjects: usize,
    /// Repeated measurements per subject.
    pub measurements: usize,
}

/// Intraclass correlation over a balanced set: one row per subject, one column per trial.
///
/// Returns `None` for fewer than two subjects or fewer than two measurements, and for a ragged
/// set, where a figure would rest on a balance the data does not have.
pub fn intraclass_correlation(
    rows: &[Vec<f64>],
    form: IntraclassForm,
) -> Option<IntraclassCorrelation> {
    let subjects = rows.len();
    if subjects < 2 {
        return None;
    }
    let measurements = rows[0].len();
    if measurements < 2 || rows.iter().any(|row| row.len() != measurements) {
        return None;
    }

    let flat: Vec<f64> = rows.iter().flatten().copied().collect();
    let grand = mean(&flat)?;
    let subject_means: Vec<f64> = rows.iter().filter_map(|row| mean(row)).collect();
    let measurement_means: Vec<f64> = (0..measurements)
        .filter_map(|column| {
            let column_values: Vec<f64> = rows.iter().map(|row| row[column]).collect();
            mean(&column_values)
        })
        .collect();

    let between_subjects = measurements as f64
        * subject_means
            .iter()
            .map(|value| (value - grand).powi(2))
            .sum::<f64>()
        / (subjects as f64 - 1.0);
    let between_measurements = subjects as f64
        * measurement_means
            .iter()
            .map(|value| (value - grand).powi(2))
            .sum::<f64>()
        / (measurements as f64 - 1.0);

    let mut residual = 0.0;
    for (index, row) in rows.iter().enumerate() {
        for (column, value) in row.iter().enumerate() {
            residual += (value - subject_means[index] - measurement_means[column] + grand).powi(2);
        }
    }
    let residual = residual / ((subjects as f64 - 1.0) * (measurements as f64 - 1.0));

    let value = match form {
        IntraclassForm::ConsistencySingle => {
            let denominator = between_subjects + (measurements as f64 - 1.0) * residual;
            if denominator == 0.0 {
                return None;
            }
            (between_subjects - residual) / denominator
        }
        IntraclassForm::AbsoluteAgreementSingle => {
            let denominator = between_subjects
                + (measurements as f64 - 1.0) * residual
                + measurements as f64 * (between_measurements - residual) / subjects as f64;
            if denominator == 0.0 {
                return None;
            }
            (between_subjects - residual) / denominator
        }
    };

    Some(IntraclassCorrelation {
        value,
        form,
        subjects,
        measurements,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pairs(values: &[(f64, f64)]) -> Vec<Pair> {
        values
            .iter()
            .map(|(first, second)| Pair {
                first: *first,
                second: *second,
            })
            .collect()
    }

    #[test]
    fn two_methods_that_never_disagree_have_no_bias_and_no_width() {
        let same = pairs(&[(1.0, 1.0), (2.0, 2.0), (3.0, 3.0)]);
        let limits = limits_of_agreement(&same, DispersionEstimator::Sample).unwrap();
        assert_eq!(limits.bias, 0.0);
        assert_eq!(limits.lower, 0.0);
        assert_eq!(limits.upper, 0.0);
        assert_eq!(limits.n, 3);
    }

    #[test]
    fn a_constant_offset_is_all_bias_and_no_width() {
        let offset = pairs(&[(1.0, 1.5), (2.0, 2.5), (3.0, 3.5)]);
        let limits = limits_of_agreement(&offset, DispersionEstimator::Sample).unwrap();
        assert!((limits.bias + 0.5).abs() < 1e-12, "{}", limits.bias);
        assert!(limits.standard_deviation_of_differences.abs() < 1e-12);
    }

    #[test]
    fn the_limits_sit_at_the_published_multiple_of_the_spread() {
        // Differences of -1, 0, 1: sample standard deviation is exactly 1.
        let spread = pairs(&[(1.0, 2.0), (2.0, 2.0), (3.0, 2.0)]);
        let limits = limits_of_agreement(&spread, DispersionEstimator::Sample).unwrap();
        assert!(limits.bias.abs() < 1e-12, "{}", limits.bias);
        assert!(
            (limits.standard_deviation_of_differences - 1.0).abs() < 1e-12,
            "{}",
            limits.standard_deviation_of_differences
        );
        assert!((limits.upper - 1.96).abs() < 1e-12, "{}", limits.upper);
        assert!((limits.lower + 1.96).abs() < 1e-12, "{}", limits.lower);
    }

    #[test]
    fn one_pair_gives_no_interval_rather_than_an_interval_of_zero() {
        assert!(limits_of_agreement(&pairs(&[(1.0, 1.0)]), DispersionEstimator::Sample).is_none());
    }

    #[test]
    fn a_perfect_straight_line_regresses_onto_itself() {
        let line = pairs(&[(1.0, 3.0), (2.0, 5.0), (3.0, 7.0), (4.0, 9.0)]);
        let fit = ordinary_least_products(&line, DispersionEstimator::Sample).unwrap();
        assert!((fit.slope - 2.0).abs() < 1e-12, "{}", fit.slope);
        assert!((fit.intercept - 1.0).abs() < 1e-12, "{}", fit.intercept);
        assert!((correlation(&line).unwrap() - 1.0).abs() < 1e-12);
    }

    #[test]
    fn a_falling_relation_keeps_its_sign() {
        let falling = pairs(&[(1.0, 9.0), (2.0, 7.0), (3.0, 5.0), (4.0, 3.0)]);
        let fit = ordinary_least_products(&falling, DispersionEstimator::Sample).unwrap();
        assert!(fit.slope < 0.0, "{}", fit.slope);
        assert!((fit.slope + 2.0).abs() < 1e-12, "{}", fit.slope);
    }

    #[test]
    fn a_coefficient_of_variation_is_the_spread_over_the_centre() {
        // Mean 10, sample standard deviation 1, so 10 percent.
        let figure =
            coefficient_of_variation(&[9.0, 10.0, 11.0], DispersionEstimator::Sample).unwrap();
        assert!((figure.percent - 10.0).abs() < 1e-12, "{}", figure.percent);
        assert_eq!(figure.n, 3);
    }

    #[test]
    fn the_two_conventions_are_two_numbers() {
        let values = [9.0, 10.0, 11.0];
        let sample = coefficient_of_variation(&values, DispersionEstimator::Sample).unwrap();
        let population =
            coefficient_of_variation(&values, DispersionEstimator::Population).unwrap();
        assert_ne!(sample.percent, population.percent);
    }

    #[test]
    fn the_mean_coefficient_is_taken_over_subjects_and_says_so() {
        let subjects = vec![vec![9.0, 10.0, 11.0], vec![18.0, 20.0, 22.0]];
        let figure = mean_coefficient_of_variation(&subjects, DispersionEstimator::Sample).unwrap();
        assert!((figure.percent - 10.0).abs() < 1e-12, "{}", figure.percent);
        assert_eq!(figure.n, 2, "the count is subjects, not trials");
    }

    #[test]
    fn identical_repeats_correlate_perfectly_and_a_ragged_set_does_not_correlate_at_all() {
        let balanced = vec![vec![1.0, 1.0], vec![5.0, 5.0], vec![9.0, 9.0]];
        let icc = intraclass_correlation(&balanced, IntraclassForm::ConsistencySingle).unwrap();
        assert!((icc.value - 1.0).abs() < 1e-12, "{}", icc.value);
        assert_eq!(icc.subjects, 3);
        assert_eq!(icc.measurements, 2);

        let ragged = vec![vec![1.0, 1.0], vec![5.0]];
        assert!(intraclass_correlation(&ragged, IntraclassForm::ConsistencySingle).is_none());
    }

    #[test]
    fn the_two_intraclass_forms_are_two_numbers_when_one_measurement_runs_high() {
        let shifted = vec![vec![1.0, 3.0], vec![5.0, 7.0], vec![9.0, 11.0]];
        let consistency =
            intraclass_correlation(&shifted, IntraclassForm::ConsistencySingle).unwrap();
        let absolute =
            intraclass_correlation(&shifted, IntraclassForm::AbsoluteAgreementSingle).unwrap();
        assert!(
            consistency.value > absolute.value,
            "consistency ignores the systematic shift that absolute agreement charges for: {} against {}",
            consistency.value,
            absolute.value
        );
    }
}
