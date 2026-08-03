//! Savitzky-Golay smoothing, and the small dense least squares it rests on.
//!
//! One onset rule in the registry tests a smoothed signal against a threshold built
//! from the raw one. Reproducing that rule needs the same filter, including its edge
//! policy, so the filter lives here rather than being approximated at the call site.

use crate::statistics::compensated_sum;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SmoothingError {
    #[error("savitzky_golay(window_length = {window_length}, polynomial_order = {polynomial_order}) requires the order to be lower than the window")]
    OrderNotBelowWindow {
        window_length: usize,
        polynomial_order: usize,
    },
    #[error("savitzky_golay(window_length = {window_length}) does not fit a trace of {sample_count} samples")]
    WindowLongerThanTrace {
        window_length: usize,
        sample_count: usize,
    },
    #[error("savitzky_golay(window_length = {window_length}, polynomial_order = {polynomial_order}) design matrix is rank deficient")]
    RankDeficient {
        window_length: usize,
        polynomial_order: usize,
    },
}

/// An orthonormal basis and the upper triangular factor that rebuilds the original.
type Decomposition = (Vec<Vec<f64>>, Vec<Vec<f64>>);

/// Thin QR by modified Gram-Schmidt with one reorthogonalisation pass, which is what
/// keeps a Vandermonde design matrix of degree three from losing half its digits.
///
/// `columns` is consumed as the matrix and returned as Q. R is returned separately in
/// row major order.
fn thin_qr(mut columns: Vec<Vec<f64>>) -> Option<Decomposition> {
    let width = columns.len();
    let mut upper = vec![vec![0.0f64; width]; width];
    for target in 0..width {
        for _pass in 0..2 {
            for earlier in 0..target {
                let projection = dot(&columns[earlier], &columns[target]);
                upper[earlier][target] += projection;
                for row in 0..columns[target].len() {
                    columns[target][row] -= projection * columns[earlier][row];
                }
            }
        }
        let norm = dot(&columns[target], &columns[target]).sqrt();
        if !(norm.is_finite() && norm > 0.0) {
            return None;
        }
        upper[target][target] = norm;
        for value in columns[target].iter_mut() {
            *value /= norm;
        }
    }
    Some((columns, upper))
}

fn dot(left: &[f64], right: &[f64]) -> f64 {
    let products: Vec<f64> = left.iter().zip(right).map(|(a, b)| a * b).collect();
    compensated_sum(&products)
}

fn solve_upper_triangular(upper: &[Vec<f64>], right_hand_side: &[f64]) -> Vec<f64> {
    let width = right_hand_side.len();
    let mut solution = vec![0.0f64; width];
    for row in (0..width).rev() {
        let mut accumulated = right_hand_side[row];
        for column in row + 1..width {
            accumulated -= upper[row][column] * solution[column];
        }
        solution[row] = accumulated / upper[row][row];
    }
    solution
}

fn solve_lower_triangular_transposed(upper: &[Vec<f64>], right_hand_side: &[f64]) -> Vec<f64> {
    let width = right_hand_side.len();
    let mut solution = vec![0.0f64; width];
    for row in 0..width {
        let mut accumulated = right_hand_side[row];
        for column in 0..row {
            accumulated -= upper[column][row] * solution[column];
        }
        solution[row] = accumulated / upper[row][row];
    }
    solution
}

fn vandermonde_columns(sample_positions: &[f64], polynomial_order: usize) -> Vec<Vec<f64>> {
    (0..=polynomial_order)
        .map(|power| {
            sample_positions
                .iter()
                .map(|position| position.powi(power as i32))
                .collect()
        })
        .collect()
}

/// Least squares polynomial fit, returning coefficients from the highest power down.
pub fn fit_polynomial(
    sample_positions: &[f64],
    observations: &[f64],
    polynomial_order: usize,
) -> Option<Vec<f64>> {
    let (orthonormal, upper) = thin_qr(vandermonde_columns(sample_positions, polynomial_order))?;
    let projected: Vec<f64> = orthonormal
        .iter()
        .map(|column| dot(column, observations))
        .collect();
    let mut ascending = solve_upper_triangular(&upper, &projected);
    ascending.reverse();
    Some(ascending)
}

/// Horner evaluation of coefficients ordered from the highest power down.
pub fn evaluate_polynomial(descending_coefficients: &[f64], position: f64) -> f64 {
    descending_coefficients
        .iter()
        .fold(0.0, |accumulated, coefficient| {
            accumulated * position + coefficient
        })
}

/// Convolution kernel that evaluates the fitted polynomial at the window centre.
///
/// An even window has no centre sample, so the evaluation point falls half a sample
/// short of one. Every jump tool in this corpus reaches this filter with an even
/// window, because a window stated in seconds times a sample rate in hundreds is even.
pub fn savitzky_golay_coefficients(
    window_length: usize,
    polynomial_order: usize,
) -> Result<Vec<f64>, SmoothingError> {
    if polynomial_order >= window_length {
        return Err(SmoothingError::OrderNotBelowWindow {
            window_length,
            polynomial_order,
        });
    }
    let half_length = window_length / 2;
    let evaluation_position = if window_length % 2 == 0 {
        half_length as f64 - 0.5
    } else {
        half_length as f64
    };
    let positions: Vec<f64> = (0..window_length)
        .rev()
        .map(|index| index as f64 - evaluation_position)
        .collect();

    // The design matrix has one row per polynomial power and one column per sample, so
    // the system is underdetermined and the wanted solution is the minimum norm one.
    let (orthonormal, upper) = thin_qr(vandermonde_columns(&positions, polynomial_order)).ok_or(
        SmoothingError::RankDeficient {
            window_length,
            polynomial_order,
        },
    )?;
    let mut unit = vec![0.0f64; polynomial_order + 1];
    unit[0] = 1.0;
    let intermediate = solve_lower_triangular_transposed(&upper, &unit);

    let mut coefficients = vec![0.0f64; window_length];
    for (column, weight) in orthonormal.iter().zip(&intermediate) {
        for (index, value) in column.iter().enumerate() {
            coefficients[index] += value * weight;
        }
    }

    // Smoothing coefficients sum to one exactly, because a constant is a polynomial of
    // degree zero and the filter fits polynomials. The least squares solve reaches that
    // only to within its own error, and the reference implementation's residual gain of
    // one part in 5e11 is enough to decide a threshold crossing on a trial whose quiet
    // window has no noise at all. Normalising removes that from the answer.
    let gain = compensated_sum(&coefficients);
    if gain != 0.0 {
        for coefficient in coefficients.iter_mut() {
            *coefficient /= gain;
        }
    }
    Ok(coefficients)
}

/// Savitzky-Golay smoothing with polynomial interpolated edges.
///
/// The edge policy is named in the function rather than defaulted, because the
/// alternatives shift the first and last half window by more than the smoothing
/// itself does, and no source in this corpus states which one it used.
pub fn savitzky_golay_interpolated_edges(
    values: &[f64],
    window_length: usize,
    polynomial_order: usize,
) -> Result<Vec<f64>, SmoothingError> {
    if window_length > values.len() {
        return Err(SmoothingError::WindowLongerThanTrace {
            window_length,
            sample_count: values.len(),
        });
    }
    let coefficients = savitzky_golay_coefficients(window_length, polynomial_order)?;
    let half_length = window_length / 2;
    let lead = window_length - 1 - half_length + usize::from(window_length % 2 == 0);

    let mut smoothed = vec![0.0f64; values.len()];
    for (output_index, slot) in smoothed.iter_mut().enumerate() {
        let mut terms = Vec::with_capacity(window_length);
        for (offset, coefficient) in coefficients.iter().enumerate() {
            let source = output_index as isize + lead as isize - offset as isize;
            if source >= 0 && (source as usize) < values.len() {
                terms.push(coefficient * values[source as usize]);
            }
        }
        *slot = compensated_sum(&terms);
    }

    // The edge fit is centred on its own window. Fitting against 0..window_length
    // instead leaves a cubic Vandermonde spanning seven orders of magnitude, which
    // costs several digits for nothing: only the evaluated value is ever used.
    let centre = (window_length - 1) as f64 / 2.0;
    let positions: Vec<f64> = (0..window_length)
        .map(|index| index as f64 - centre)
        .collect();
    let rank_deficient = || SmoothingError::RankDeficient {
        window_length,
        polynomial_order,
    };

    let leading = fit_polynomial(&positions, &values[..window_length], polynomial_order)
        .ok_or_else(rank_deficient)?;
    for (index, slot) in smoothed.iter_mut().enumerate().take(half_length) {
        *slot = evaluate_polynomial(&leading, index as f64 - centre);
    }

    let tail_start = values.len() - window_length;
    let trailing = fit_polynomial(&positions, &values[tail_start..], polynomial_order)
        .ok_or_else(rank_deficient)?;
    for (index, slot) in smoothed
        .iter_mut()
        .enumerate()
        .skip(values.len() - half_length)
    {
        *slot = evaluate_polynomial(&trailing, (index - tail_start) as f64 - centre);
    }

    Ok(smoothed)
}

/// A centred rectangular moving average, which is this filter at polynomial order zero.
///
/// Fitting a degree-zero polynomial over a window and evaluating it at the centre is the
/// window's arithmetic mean, so the boxcar is a named entry point onto the filter above
/// rather than a convolution of its own. Two implementations would disagree at the ends,
/// where this one continues the edge fit and a hand-written boxcar would truncate the
/// window and silently change the divisor.
pub fn moving_average_boxcar(
    values: &[f64],
    window_length: usize,
) -> Result<Vec<f64>, SmoothingError> {
    savitzky_golay_interpolated_edges(values, window_length, 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Reference values from `scipy.signal.savgol_coeffs`, which is the implementation
    /// every Python tool in the registry reaches through.
    #[test]
    fn coefficients_match_the_reference_for_an_odd_window() {
        let coefficients = savitzky_golay_coefficients(5, 2).unwrap();
        let expected = [
            -0.085_714_285_714_285_7,
            0.342_857_142_857_142_8,
            0.485_714_285_714_285_7,
            0.342_857_142_857_142_8,
            -0.085_714_285_714_285_7,
        ];
        for (got, want) in coefficients.iter().zip(expected) {
            assert!((got - want).abs() < 1e-12, "{got} against {want}");
        }
    }

    #[test]
    fn coefficients_match_the_reference_for_an_even_window() {
        let coefficients = savitzky_golay_coefficients(6, 3).unwrap();
        let expected = [-0.093_75, 0.218_75, 0.375, 0.375, 0.218_75, -0.093_75];
        for (got, want) in coefficients.iter().zip(expected) {
            assert!((got - want).abs() < 1e-12, "{got} against {want}");
        }
    }

    fn cubic(position: f64) -> f64 {
        let t = position / 50.0;
        2.0 + 3.0 * t - 1.5 * t * t + 0.25 * t * t * t
    }

    /// A polynomial of degree at most the filter order passes through unchanged, which
    /// is the defining property and covers the interior and both edge fits at once.
    #[test]
    fn a_cubic_passes_through_an_odd_cubic_filter_unchanged() {
        let values: Vec<f64> = (0..500).map(|i| cubic(i as f64)).collect();
        let smoothed = savitzky_golay_interpolated_edges(&values, 241, 3).unwrap();
        for (index, (got, want)) in smoothed.iter().zip(&values).enumerate() {
            assert!(
                (got - want).abs() < 1e-6,
                "sample {index}: {got} against {want}"
            );
        }
    }

    /// An even window has no centre sample, so it estimates the value half a sample
    /// later than the sample it is written to. Every jump tool in this corpus reaches
    /// this filter with an even window, because a window stated in tenths of a second
    /// times a sample rate in hundreds is even.
    #[test]
    fn an_even_window_shifts_the_signal_half_a_sample_later() {
        let values: Vec<f64> = (0..500).map(|i| cubic(i as f64)).collect();
        let smoothed = savitzky_golay_interpolated_edges(&values, 240, 3).unwrap();
        for index in 120..380 {
            let shifted = cubic(index as f64 + 0.5);
            assert!(
                (smoothed[index] - shifted).abs() < 1e-6,
                "sample {index}: {} against {shifted}",
                smoothed[index]
            );
            assert!(
                (smoothed[index] - values[index]).abs() > 1e-9,
                "sample {index} came back centred"
            );
        }
    }

    /// Reference values from `scipy.signal.savgol_filter(values, 240, 3)`, which is the
    /// call every Python tool in the registry makes.
    #[test]
    fn the_even_window_filter_matches_the_reference_implementation() {
        let values: Vec<f64> = (0..500).map(|i| cubic(i as f64)).collect();
        let smoothed = savitzky_golay_interpolated_edges(&values, 240, 3).unwrap();
        for (index, expected) in [
            (120usize, 4.017_230_250_008_357),
            (250, 10.817_725_250_022_502),
        ] {
            assert!(
                (smoothed[index] - expected).abs() < 1e-9,
                "sample {index}: {} against {expected}",
                smoothed[index]
            );
        }
    }

    /// A constant is a polynomial of degree zero, so it must come back unchanged. On a
    /// trial whose quiet window is one repeated value, this property is the entire
    /// difference between a threshold that fires and one that does not, so the interior
    /// is held to equality rather than to a tolerance.
    #[test]
    fn a_constant_comes_back_as_the_same_constant() {
        let values = vec![665.430_3f64; 3000];
        let smoothed = savitzky_golay_interpolated_edges(&values, 240, 3).unwrap();
        for (index, &got) in smoothed.iter().enumerate().take(2880).skip(120) {
            assert_eq!(got, 665.430_3, "sample {index} came back as {got}");
        }
        for (index, &got) in smoothed.iter().enumerate() {
            assert!(
                (got - 665.430_3).abs() < 1e-11,
                "edge sample {index} came back as {got}"
            );
        }
    }

    /// The boxcar is held to the arithmetic mean over the window, computed here rather
    /// than taken from the filter it calls, so the check is against the definition and not
    /// against itself. An odd window is centred on its own sample.
    #[test]
    fn the_boxcar_is_the_arithmetic_mean_over_an_odd_window() {
        let values: Vec<f64> = (0..500)
            .map(|i| cubic(i as f64) + ((i % 7) as f64) * 0.3)
            .collect();
        let width = 41usize;
        let averaged = moving_average_boxcar(&values, width).unwrap();
        for centre in width..500 - width {
            let window = &values[centre - width / 2..=centre + width / 2];
            let mean = window.iter().sum::<f64>() / width as f64;
            assert!(
                (averaged[centre] - mean).abs() < 1e-9,
                "sample {centre}: {} against {mean}",
                averaged[centre]
            );
        }
    }

    /// Every jump tool in this corpus reaches a moving average with an even window,
    /// because a width in seconds times a sample rate in hundreds is even. An even window
    /// has no centre sample, so the mean it returns is the one taken half a sample later.
    #[test]
    fn an_even_boxcar_window_returns_the_mean_taken_half_a_sample_later() {
        let values: Vec<f64> = (0..500)
            .map(|i| cubic(i as f64) + ((i % 7) as f64) * 0.3)
            .collect();
        let width = 40usize;
        let averaged = moving_average_boxcar(&values, width).unwrap();
        for centre in width..500 - width {
            let window = &values[centre - width / 2 + 1..=centre + width / 2];
            let mean = window.iter().sum::<f64>() / width as f64;
            assert!(
                (averaged[centre] - mean).abs() < 1e-9,
                "sample {centre}: {} against {mean}",
                averaged[centre]
            );
        }
    }

    /// The interior kernels of order zero and order one are the same arithmetic, because a
    /// least-squares line evaluated at the centre of a symmetric window is that window's
    /// mean. The two part company at the ends, where order zero holds the leading half
    /// window at one level and order one runs a slope through it. So the edge is where the
    /// boxcar's policy is actually pinned, and an interior check alone cannot tell a
    /// boxcar from a local linear fit.
    #[test]
    fn the_leading_and_trailing_half_windows_hold_one_level() {
        let values: Vec<f64> = (0..500).map(|i| cubic(i as f64)).collect();
        let width = 41usize;
        let averaged = moving_average_boxcar(&values, width).unwrap();

        let leading_mean = values[..width].iter().sum::<f64>() / width as f64;
        for (index, &got) in averaged.iter().enumerate().take(width / 2) {
            assert!(
                (got - leading_mean).abs() < 1e-9,
                "leading sample {index} came back as {got} against {leading_mean}"
            );
        }

        let trailing_mean = values[500 - width..].iter().sum::<f64>() / width as f64;
        for (index, &got) in averaged.iter().enumerate().skip(500 - width / 2) {
            assert!(
                (got - trailing_mean).abs() < 1e-9,
                "trailing sample {index} came back as {got} against {trailing_mean}"
            );
        }
    }

    #[test]
    fn a_window_longer_than_the_trace_names_the_window_and_the_trace() {
        let error = savitzky_golay_interpolated_edges(&[1.0, 2.0, 3.0], 240, 3).unwrap_err();
        let message = error.to_string();
        assert!(message.contains("240"), "{message}");
        assert!(message.contains("3 samples"), "{message}");
    }
}
