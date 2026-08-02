//! A b-spline basis, and least squares onto it.
//!
//! Expressing a curve in a basis is an alternative to resampling it onto a percentage
//! grid: the curve becomes a short vector of coefficients rather than a long vector of
//! points. The number of basis functions sets how much of the curve survives, so it is a
//! smoothing choice wearing the clothes of a representation choice, and the registry entry
//! that asks for this basis records that the interaction with the acquisition filter is
//! undiscussed in its literature.

#[derive(Debug, thiserror::Error)]
pub enum BSplineError {
    #[error("bspline(basis_count = {basis_count}, degree = {degree}) needs at least degree + 1 basis functions")]
    BasisTooSmall { basis_count: usize, degree: usize },
    #[error("bspline fit needs at least as many observations as basis functions: {observation_count} observations against {basis_count} functions")]
    FewerObservationsThanFunctions {
        observation_count: usize,
        basis_count: usize,
    },
    #[error("bspline fit reached a rank deficient normal matrix at basis function {index}")]
    RankDeficient { index: usize },
}

/// A clamped uniform b-spline basis over the unit interval.
///
/// Clamped means the end knots are repeated, so the first and last basis functions reach
/// one at the ends and the fitted curve interpolates the ends of its own span rather than
/// tapering away from them.
#[derive(Debug, Clone)]
pub struct Basis {
    degree: usize,
    knots: Vec<f64>,
    basis_count: usize,
}

impl Basis {
    pub fn clamped_uniform(basis_count: usize, degree: usize) -> Result<Self, BSplineError> {
        if basis_count < degree + 1 {
            return Err(BSplineError::BasisTooSmall {
                basis_count,
                degree,
            });
        }
        let interior_count = basis_count - degree - 1;
        let mut knots = vec![0.0f64; degree + 1];
        knots.extend((1..=interior_count).map(|step| step as f64 / (interior_count + 1) as f64));
        knots.extend(std::iter::repeat_n(1.0, degree + 1));
        Ok(Self {
            degree,
            knots,
            basis_count,
        })
    }

    pub fn basis_count(&self) -> usize {
        self.basis_count
    }

    /// Every basis function evaluated at one position in the unit interval, by the
    /// Cox-de Boor recursion.
    pub fn at(&self, position: f64) -> Vec<f64> {
        let clamped = position.clamp(0.0, 1.0);
        let mut current = vec![0.0f64; self.knots.len() - 1];
        for (index, span) in current.iter_mut().enumerate() {
            let inside = self.knots[index] <= clamped && clamped < self.knots[index + 1];
            // The closed right end belongs to the last span that is not degenerate, or the
            // curve would evaluate to zero everywhere at position one.
            let closing = clamped >= 1.0
                && self.knots[index] < self.knots[index + 1]
                && self.knots[index + 1] >= 1.0;
            *span = f64::from(inside || closing);
        }

        for order in 1..=self.degree {
            let previous = current.clone();
            for index in 0..current.len() - order {
                let left_span = self.knots[index + order] - self.knots[index];
                let right_span = self.knots[index + order + 1] - self.knots[index + 1];
                let rising = if left_span > 0.0 {
                    (clamped - self.knots[index]) / left_span * previous[index]
                } else {
                    0.0
                };
                let falling = if right_span > 0.0 {
                    (self.knots[index + order + 1] - clamped) / right_span * previous[index + 1]
                } else {
                    0.0
                };
                current[index] = rising + falling;
            }
        }
        current.truncate(self.basis_count);
        current
    }

    /// The design matrix over positions spread evenly across the unit interval, which is
    /// what a curve already resampled onto a percentage grid sits on.
    pub fn design_over_evenly_spaced(&self, point_count: usize) -> Vec<Vec<f64>> {
        (0..point_count)
            .map(|point| self.at(point as f64 / (point_count - 1).max(1) as f64))
            .collect()
    }

    /// Least squares coefficients for one curve, by Cholesky on the normal equations.
    ///
    /// A b-spline design matrix is banded and well conditioned, which is the property that
    /// makes the normal equations safe here and unsafe for the monomials in `smoothing`.
    pub fn fit(&self, observations: &[f64]) -> Result<Vec<f64>, BSplineError> {
        if observations.len() < self.basis_count {
            return Err(BSplineError::FewerObservationsThanFunctions {
                observation_count: observations.len(),
                basis_count: self.basis_count,
            });
        }
        let design = self.design_over_evenly_spaced(observations.len());

        let mut normal = vec![vec![0.0f64; self.basis_count]; self.basis_count];
        let mut projected = vec![0.0f64; self.basis_count];
        for (row, observation) in design.iter().zip(observations) {
            for left in 0..self.basis_count {
                projected[left] += row[left] * observation;
                for right in 0..=left {
                    normal[left][right] += row[left] * row[right];
                }
            }
        }

        for column in 0..self.basis_count {
            let settled: Vec<f64> = normal[column][..column].to_vec();
            let pivot =
                normal[column][column] - settled.iter().map(|value| value * value).sum::<f64>();
            if !(pivot.is_finite() && pivot > 0.0) {
                return Err(BSplineError::RankDeficient { index: column });
            }
            let root = pivot.sqrt();
            normal[column][column] = root;
            for row_values in normal.iter_mut().skip(column + 1) {
                let crossed: f64 = row_values[..column]
                    .iter()
                    .zip(&settled)
                    .map(|(left, right)| left * right)
                    .sum();
                row_values[column] = (row_values[column] - crossed) / root;
            }
        }

        let mut intermediate: Vec<f64> = Vec::with_capacity(self.basis_count);
        for (row_values, &projection) in normal.iter().zip(&projected) {
            let solved_so_far = intermediate.len();
            let crossed: f64 = row_values[..solved_so_far]
                .iter()
                .zip(&intermediate)
                .map(|(factor, solved)| factor * solved)
                .sum();
            intermediate.push((projection - crossed) / row_values[solved_so_far]);
        }

        let mut coefficients = vec![0.0f64; self.basis_count];
        for row in (0..self.basis_count).rev() {
            let crossed: f64 = normal[row + 1..]
                .iter()
                .zip(&coefficients[row + 1..])
                .map(|(later_row, solved)| later_row[row] * solved)
                .sum();
            coefficients[row] = (intermediate[row] - crossed) / normal[row][row];
        }
        Ok(coefficients)
    }

    /// The curve those coefficients describe, back on a grid of stated length.
    pub fn evaluate(&self, coefficients: &[f64], point_count: usize) -> Vec<f64> {
        self.design_over_evenly_spaced(point_count)
            .into_iter()
            .map(|row| {
                row.iter()
                    .zip(coefficients)
                    .map(|(weight, coefficient)| weight * coefficient)
                    .sum()
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A b-spline basis is a partition of unity, so its functions sum to one everywhere in
    /// the span. It is the cheapest check that the recursion and the knot vector agree,
    /// and it fails on an off-by-one in either.
    #[test]
    fn the_basis_functions_sum_to_one_across_the_span() {
        let basis = Basis::clamped_uniform(12, 3).unwrap();
        for step in 0..=100 {
            let position = step as f64 / 100.0;
            let total: f64 = basis.at(position).iter().sum();
            assert!(
                (total - 1.0).abs() < 1e-12,
                "at {position} the basis summed to {total}"
            );
        }
    }

    /// The defining property, matching the resampler's: a polynomial of degree at or below
    /// the spline degree lies in the span of the basis, so the fit returns it and not an
    /// approximation of it. This covers the interior and both clamped ends at once.
    #[test]
    fn a_cubic_is_returned_unchanged_by_a_cubic_basis() {
        let point_count = 400usize;
        let cubic =
            |position: f64| 7.0 - 2.5 * position + 1.25 * position.powi(2) - 0.4 * position.powi(3);
        let observations: Vec<f64> = (0..point_count)
            .map(|point| cubic(point as f64 / (point_count - 1) as f64))
            .collect();

        let basis = Basis::clamped_uniform(10, 3).unwrap();
        let coefficients = basis.fit(&observations).unwrap();
        let rebuilt = basis.evaluate(&coefficients, point_count);
        for (point, (&got, &want)) in rebuilt.iter().zip(&observations).enumerate() {
            assert!(
                (got - want).abs() < 1e-9,
                "point {point}: {got} against {want}"
            );
        }
    }

    /// Basis size is the smoothing dial the entry says it is: a small basis cannot follow
    /// a sharp feature and a large one can, on the same curve.
    #[test]
    fn the_basis_size_decides_how_much_of_a_sharp_feature_survives() {
        let point_count = 400usize;
        let observations: Vec<f64> = (0..point_count)
            .map(|point| {
                let position = point as f64 / (point_count - 1) as f64;
                if (0.48..0.52).contains(&position) {
                    1.0
                } else {
                    0.0
                }
            })
            .collect();

        let peak_under = |basis_count: usize| {
            let basis = Basis::clamped_uniform(basis_count, 3).unwrap();
            let coefficients = basis.fit(&observations).unwrap();
            basis
                .evaluate(&coefficients, point_count)
                .into_iter()
                .fold(f64::NEG_INFINITY, f64::max)
        };
        let coarse = peak_under(8);
        let fine = peak_under(60);
        assert!(
            fine > coarse * 2.0,
            "a basis of 60 peaked at {fine} and a basis of 8 at {coarse}"
        );
    }

    #[test]
    fn a_basis_smaller_than_the_degree_allows_names_both_numbers() {
        let error = Basis::clamped_uniform(3, 3).unwrap_err();
        let message = error.to_string();
        assert!(message.contains('3'), "{message}");
    }

    #[test]
    fn fewer_observations_than_functions_names_both_counts() {
        let basis = Basis::clamped_uniform(12, 3).unwrap();
        let error = basis.fit(&[1.0, 2.0, 3.0]).unwrap_err();
        let message = error.to_string();
        assert!(message.contains("12"), "{message}");
        assert!(message.contains('3'), "{message}");
    }
}
