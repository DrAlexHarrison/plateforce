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

/// A smoothed curve, and how much of the data it kept.
///
/// The penalty weight is not a number a user would recognise, because it is a property of
/// this basis rather than of the signal. What travels beside it is the effective degrees of
/// freedom, which says how many free parameters the fit actually spent, and the score the
/// weight was chosen by.
#[derive(Debug, Clone)]
pub struct PenalisedFit {
    pub coefficients: Vec<f64>,
    pub fitted: Vec<f64>,
    pub penalty_weight: f64,
    pub effective_degrees_of_freedom: f64,
    pub cross_validation_score: f64,
}

/// Lower triangular Cholesky factor of a symmetric positive definite matrix, in place of
/// the matrix. Nothing when the matrix is not positive definite, which for a penalised
/// normal matrix means the basis is larger than the data can support.
fn cholesky(matrix: &[Vec<f64>]) -> Option<Vec<Vec<f64>>> {
    let mut factor = matrix.to_vec();
    let width = factor.len();
    for column in 0..width {
        let settled: Vec<f64> = factor[column][..column].to_vec();
        let pivot = factor[column][column] - settled.iter().map(|value| value * value).sum::<f64>();
        if !(pivot.is_finite() && pivot > 0.0) {
            return None;
        }
        let root = pivot.sqrt();
        factor[column][column] = root;
        for row_values in factor.iter_mut().skip(column + 1) {
            let crossed: f64 = row_values[..column]
                .iter()
                .zip(&settled)
                .map(|(left, right)| left * right)
                .sum();
            row_values[column] = (row_values[column] - crossed) / root;
        }
    }
    Some(factor)
}

/// Forward then back substitution through a Cholesky factor.
fn solve_with(factor: &[Vec<f64>], right_hand_side: &[f64]) -> Vec<f64> {
    let width = factor.len();
    let mut intermediate: Vec<f64> = Vec::with_capacity(width);
    for (row_values, &value) in factor.iter().zip(right_hand_side) {
        let solved_so_far = intermediate.len();
        let crossed: f64 = row_values[..solved_so_far]
            .iter()
            .zip(&intermediate)
            .map(|(coefficient, solved)| coefficient * solved)
            .sum();
        intermediate.push((value - crossed) / row_values[solved_so_far]);
    }
    let mut solution = vec![0.0f64; width];
    for row in (0..width).rev() {
        let crossed: f64 = factor[row + 1..]
            .iter()
            .zip(&solution[row + 1..])
            .map(|(later_row, solved)| later_row[row] * solved)
            .sum();
        solution[row] = (intermediate[row] - crossed) / factor[row][row];
    }
    solution
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

    /// Least squares coefficients for one curve.
    ///
    /// The unpenalised fit is the penalised one at a weight of zero, so there is one solver
    /// and the penalty is a value rather than a second routine.
    pub fn fit(&self, observations: &[f64]) -> Result<Vec<f64>, BSplineError> {
        Ok(self.fit_penalised(observations, 0.0)?.coefficients)
    }

    /// Least squares with a second-difference penalty on the coefficients, which is the
    /// discrete form of penalising how much the curve bends.
    ///
    /// A weight of zero returns the ordinary fit, so the penalised and unpenalised routes
    /// are one implementation and the penalty is a value rather than a branch.
    pub fn fit_penalised(
        &self,
        observations: &[f64],
        penalty_weight: f64,
    ) -> Result<PenalisedFit, BSplineError> {
        if observations.len() < self.basis_count {
            return Err(BSplineError::FewerObservationsThanFunctions {
                observation_count: observations.len(),
                basis_count: self.basis_count,
            });
        }
        let design = self.design_over_evenly_spaced(observations.len());
        let width = self.basis_count;

        let mut gram = vec![vec![0.0f64; width]; width];
        let mut projected = vec![0.0f64; width];
        for (row, observation) in design.iter().zip(observations) {
            for left in 0..width {
                projected[left] += row[left] * observation;
                for right in 0..width {
                    gram[left][right] += row[left] * row[right];
                }
            }
        }

        let mut penalised = gram.clone();
        for start in 0..width.saturating_sub(2) {
            // One row of the second-difference operator, [1, -2, 1], contributing its outer
            // product to the penalty.
            let stencil = [(start, 1.0), (start + 1, -2.0), (start + 2, 1.0)];
            for (left, left_weight) in stencil {
                for (right, right_weight) in stencil {
                    penalised[left][right] += penalty_weight * left_weight * right_weight;
                }
            }
        }

        let factored = cholesky(&penalised).ok_or(BSplineError::RankDeficient { index: 0 })?;
        let coefficients = solve_with(&factored, &projected);

        // The fit's trace, which is how many parameters it actually spent. Taken as
        // trace of the penalised inverse times the unpenalised Gram, one column at a time,
        // over a matrix the size of the basis rather than of the trace.
        let mut effective_degrees_of_freedom = 0.0f64;
        for column in 0..width {
            let unit: Vec<f64> = gram.iter().map(|row| row[column]).collect();
            effective_degrees_of_freedom += solve_with(&factored, &unit)[column];
        }

        let fitted: Vec<f64> = design
            .iter()
            .map(|row| {
                row.iter()
                    .zip(&coefficients)
                    .map(|(weight, coefficient)| weight * coefficient)
                    .sum()
            })
            .collect();
        let residual_sum_of_squares: f64 = fitted
            .iter()
            .zip(observations)
            .map(|(got, want)| (got - want).powi(2))
            .sum();

        let count = observations.len() as f64;
        let slack = count - effective_degrees_of_freedom;
        let cross_validation_score = if slack > 0.0 {
            count * residual_sum_of_squares / (slack * slack)
        } else {
            f64::INFINITY
        };

        Ok(PenalisedFit {
            coefficients,
            fitted,
            penalty_weight,
            effective_degrees_of_freedom,
            cross_validation_score,
        })
    }

    /// The penalty weight that minimises the generalised cross-validation score.
    ///
    /// Searched over a logarithmic grid and then refined around the winner, because the
    /// score is smooth in the logarithm of the weight and flat in the weight itself. The
    /// criterion chooses the smoothing rather than the operator choosing it, which is the
    /// whole content of the published rule: nobody states a cutoff and the data does.
    pub fn choose_penalty_by_cross_validation(
        &self,
        observations: &[f64],
    ) -> Result<PenalisedFit, BSplineError> {
        let mut best: Option<PenalisedFit> = None;
        let consider = |weight: f64, best: &mut Option<PenalisedFit>| {
            if let Ok(fit) = self.fit_penalised(observations, weight) {
                let better = best
                    .as_ref()
                    .is_none_or(|held| fit.cross_validation_score < held.cross_validation_score);
                if better {
                    *best = Some(fit);
                }
            }
        };

        for exponent in -10..=10 {
            consider(10.0f64.powi(exponent), &mut best);
        }
        let coarse = best
            .as_ref()
            .ok_or(BSplineError::RankDeficient { index: 0 })?
            .penalty_weight;
        for step in 1..10 {
            let fraction = step as f64 / 10.0;
            consider(coarse * 10.0f64.powf(fraction - 0.5), &mut best);
        }
        best.ok_or(BSplineError::RankDeficient { index: 0 })
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

    /// The property that defines cross-validated smoothing: given the same underlying
    /// curve, the criterion spends fewer parameters on the noisier copy. If it did not,
    /// the criterion is not reaching the answer and the penalty is decorative.
    #[test]
    fn the_criterion_spends_fewer_parameters_on_a_noisier_copy() {
        let point_count = 400usize;
        let underlying = |point: usize| {
            let position = point as f64 / (point_count - 1) as f64;
            (2.0 * std::f64::consts::PI * position).sin()
        };
        let clean: Vec<f64> = (0..point_count).map(underlying).collect();
        let noisy: Vec<f64> = (0..point_count)
            .map(|point| underlying(point) + ((point % 13) as f64 - 6.0) * 0.08)
            .collect();

        let basis = Basis::clamped_uniform(24, 3).unwrap();
        let on_clean = basis.choose_penalty_by_cross_validation(&clean).unwrap();
        let on_noisy = basis.choose_penalty_by_cross_validation(&noisy).unwrap();
        assert!(
            on_noisy.effective_degrees_of_freedom < on_clean.effective_degrees_of_freedom,
            "noisy spent {} parameters and clean spent {}",
            on_noisy.effective_degrees_of_freedom,
            on_clean.effective_degrees_of_freedom
        );
    }

    /// A weight of zero is the ordinary fit, which is what lets the two share one solver.
    /// The effective degrees of freedom then equal the basis size, because nothing is
    /// being held back.
    #[test]
    fn a_zero_penalty_spends_the_whole_basis() {
        let point_count = 400usize;
        let observations: Vec<f64> = (0..point_count)
            .map(|point| (point as f64 / 40.0).sin())
            .collect();
        let basis = Basis::clamped_uniform(16, 3).unwrap();
        let unpenalised = basis.fit_penalised(&observations, 0.0).unwrap();
        assert!(
            (unpenalised.effective_degrees_of_freedom - 16.0).abs() < 1e-6,
            "spent {}",
            unpenalised.effective_degrees_of_freedom
        );
        let heavily = basis.fit_penalised(&observations, 1e6).unwrap();
        assert!(
            heavily.effective_degrees_of_freedom < 4.0,
            "a heavy penalty still spent {}",
            heavily.effective_degrees_of_freedom
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
