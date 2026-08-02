//! Interpolating cubic spline resampling onto a common number of points.
//!
//! Time normalisation turns a timebase in seconds into one in percent of movement, and the
//! rule that asks for it says interpolating spline rather than straight lines. The two are
//! not interchangeable: linear interpolation flattens a peak that falls between samples,
//! and a peak between samples is what a rate of force development is taken across.
//!
//! The end condition is not-a-knot, which is the one that reproduces a cubic exactly. The
//! natural condition forces the second derivative to zero at both ends and so bends the
//! first and last segment away from the data, which on a force trace is the segment
//! carrying takeoff.

#[derive(Debug, thiserror::Error)]
pub enum ResampleError {
    #[error("resample needs at least 4 samples for a not-a-knot cubic spline and the interval holds {sample_count}")]
    IntervalTooShort { sample_count: usize },
    #[error("resample(from_index = {from_index}, to_index = {to_index}) needs the interval to run forwards and to end inside a trace of {sample_count} samples")]
    IntervalOutsideTrace {
        from_index: usize,
        to_index: usize,
        sample_count: usize,
    },
    #[error("resample(point_count = {point_count}) needs at least two points")]
    PointCountBelowTwo { point_count: usize },
}

/// A cubic spline through every sample, held as the second derivative at each knot.
///
/// Knots sit one index apart, which is what a uniformly sampled trace gives, so the
/// spacing does not travel in the type and every position below is in samples.
#[derive(Debug, Clone)]
pub struct CubicSpline {
    values: Vec<f64>,
    second_derivatives: Vec<f64>,
}

impl CubicSpline {
    /// Fit through the samples, with the third derivative continuous across the second and
    /// second-to-last knot.
    pub fn through(values: &[f64]) -> Result<Self, ResampleError> {
        let count = values.len();
        if count < 4 {
            return Err(ResampleError::IntervalTooShort {
                sample_count: count,
            });
        }

        // Under unit spacing the not-a-knot row at each end collapses into its neighbour
        // and hands back that neighbour's moment directly, leaving an ordinary tridiagonal
        // system across the samples between them.
        let curvature =
            |index: usize| 6.0 * (values[index + 1] - 2.0 * values[index] + values[index - 1]);
        let mut moments = vec![0.0f64; count];
        moments[1] = curvature(1) / 6.0;
        moments[count - 2] = curvature(count - 2) / 6.0;

        if count > 4 {
            let interior = count - 4;
            let mut diagonal = vec![4.0f64; interior];
            let mut right_hand_side: Vec<f64> = (2..count - 2).map(curvature).collect();
            right_hand_side[0] -= moments[1];
            right_hand_side[interior - 1] -= moments[count - 2];

            for row in 1..interior {
                let factor = 1.0 / diagonal[row - 1];
                diagonal[row] -= factor;
                right_hand_side[row] -= factor * right_hand_side[row - 1];
            }
            moments[count - 3] = right_hand_side[interior - 1] / diagonal[interior - 1];
            for row in (0..interior - 1).rev() {
                moments[row + 2] = (right_hand_side[row] - moments[row + 3]) / diagonal[row];
            }
        }

        moments[0] = 2.0 * moments[1] - moments[2];
        moments[count - 1] = 2.0 * moments[count - 2] - moments[count - 3];

        Ok(Self {
            values: values.to_vec(),
            second_derivatives: moments,
        })
    }

    /// The curve at a position in samples, which need not be a whole number.
    pub fn at(&self, position_in_samples: f64) -> f64 {
        let last = self.values.len() - 1;
        let clamped = position_in_samples.clamp(0.0, last as f64);
        let left = (clamped.floor() as usize).min(last.saturating_sub(1));
        let offset = clamped - left as f64;
        let complement = 1.0 - offset;
        let (curvature_left, curvature_right) = (
            self.second_derivatives[left],
            self.second_derivatives[left + 1],
        );
        curvature_left * complement.powi(3) / 6.0
            + curvature_right * offset.powi(3) / 6.0
            + (self.values[left] - curvature_left / 6.0) * complement
            + (self.values[left + 1] - curvature_right / 6.0) * offset
    }
}

/// Resample the interval between two samples onto a stated number of evenly spaced points.
///
/// Both index bounds are included, so the first returned point is the value at
/// `from_index` and the last is the value at `to_index`, and the timebase between them
/// becomes a proportion of the movement rather than an elapsed time.
pub fn resample_interval(
    values: &[f64],
    from_index: usize,
    to_index: usize,
    point_count: usize,
) -> Result<Vec<f64>, ResampleError> {
    if point_count < 2 {
        return Err(ResampleError::PointCountBelowTwo { point_count });
    }
    if to_index <= from_index || to_index >= values.len() {
        return Err(ResampleError::IntervalOutsideTrace {
            from_index,
            to_index,
            sample_count: values.len(),
        });
    }
    let spline = CubicSpline::through(&values[from_index..=to_index])?;
    let span = (to_index - from_index) as f64;
    Ok((0..point_count)
        .map(|point| spline.at(span * point as f64 / (point_count - 1) as f64))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cubic(position: f64) -> f64 {
        let scaled = position / 100.0;
        7.0 - 2.5 * scaled + 1.25 * scaled * scaled - 0.4 * scaled * scaled * scaled
    }

    /// The defining property: a cubic spline through samples of a cubic is that cubic, so
    /// every resampled point sits on it.
    ///
    /// The grid is finer than the samples on purpose. A grid coarser than one point per
    /// segment lands on the end knots and nowhere between them, and at a knot the spline
    /// interpolates whatever the end condition was, so a coarse grid cannot tell not-a-knot
    /// from the natural condition that bends the first and last segment away from the data.
    #[test]
    fn a_cubic_comes_back_unchanged_at_every_resampled_point() {
        let values: Vec<f64> = (0..600).map(|index| cubic(index as f64)).collect();
        let point_count = 1198usize;
        let resampled = resample_interval(&values, 0, 599, point_count).unwrap();
        for (point, &got) in resampled.iter().enumerate() {
            let position = 599.0 * point as f64 / (point_count - 1) as f64;
            let want = cubic(position);
            assert!(
                (got - want).abs() < 1e-8,
                "point {point} at sample {position}: {got} against {want}"
            );
        }
    }

    /// The end condition stated directly, at the one place it is visible: between the first
    /// two knots and between the last two. Both are exact under not-a-knot and neither is
    /// under the natural condition.
    #[test]
    fn the_two_end_segments_reproduce_the_cubic_between_their_knots() {
        let values: Vec<f64> = (0..600).map(|index| cubic(index as f64)).collect();
        let spline = CubicSpline::through(&values).unwrap();
        for position in [0.25f64, 0.5, 0.75, 597.25, 597.5, 598.75] {
            let want = cubic(position);
            let got = spline.at(position);
            assert!(
                (got - want).abs() < 1e-8,
                "sample {position}: {got} against {want}"
            );
        }
    }

    #[test]
    fn the_endpoints_are_the_samples_at_the_interval_bounds() {
        let values: Vec<f64> = (0..600).map(|index| cubic(index as f64)).collect();
        let resampled = resample_interval(&values, 120, 480, 51).unwrap();
        assert!((resampled[0] - values[120]).abs() < 1e-12);
        assert!((resampled[50] - values[480]).abs() < 1e-12);
        assert_eq!(resampled.len(), 51);
    }

    /// A peak that falls between two samples is what separates a spline from straight
    /// lines, and it is the shape a rate is taken across.
    #[test]
    fn a_peak_between_samples_survives_the_spline_and_not_the_straight_line() {
        let values: Vec<f64> = (0..401)
            .map(|index| {
                let seconds = index as f64 / 400.0;
                (std::f64::consts::PI * seconds).sin()
            })
            .collect();
        let spline = CubicSpline::through(&values).unwrap();
        let between = spline.at(200.5);
        let straight = (values[200] + values[201]) / 2.0;
        let exact = (std::f64::consts::PI * 200.5 / 400.0).sin();
        assert!((between - exact).abs() < (straight - exact).abs());
    }

    #[test]
    fn an_interval_shorter_than_the_spline_needs_names_its_length() {
        let values = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let error = resample_interval(&values, 0, 2, 10).unwrap_err();
        assert!(error.to_string().contains('3'), "{error}");
    }

    #[test]
    fn an_interval_running_off_the_trace_names_both_bounds() {
        let values: Vec<f64> = (0..100).map(|index| index as f64).collect();
        let error = resample_interval(&values, 10, 400, 10).unwrap_err();
        let message = error.to_string();
        assert!(message.contains("400"), "{message}");
        assert!(message.contains("100"), "{message}");
    }
}
