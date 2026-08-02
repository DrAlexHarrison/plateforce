//! Treating a whole force-time curve as the observation, rather than a scalar taken off it.
//!
//! Every rule here consumes a set of curves already on a common timebase. Which rule put
//! them on that timebase changes the answer and is not decided here: a caller passes curves
//! a bound time treatment produced, and the rule that produced them travels beside the
//! result rather than being inferred from the shape of the data.
//!
//! The decomposition below is shared by every component-based rule in this module. Two
//! implementations of an eigendecomposition would be two answers to one question, so the
//! rules differ in what they decompose and in what they do afterwards, never in how.

use crate::statistics::compensated_sum;

#[derive(Debug, thiserror::Error)]
pub enum WaveformError {
    #[error("a curve set needs at least two curves and holds {curve_count}")]
    FewerThanTwoCurves { curve_count: usize },
    #[error("curve {index} holds {point_count} points and the first holds {expected}")]
    RaggedCurveSet {
        index: usize,
        point_count: usize,
        expected: usize,
    },
    #[error("a curve needs at least two points and holds {point_count}")]
    CurveTooShort { point_count: usize },
    #[error("variance_retained_pct is {value} and a share of the variance lies in (0, 100]")]
    VarianceShareOutsideRange { value: f64 },
    #[error("the decomposition did not converge in {sweeps} sweeps")]
    DecompositionDidNotConverge { sweeps: usize },
    #[error("phase_loading_threshold is {value} and a share of a component's peak loading lies in (0, 1]")]
    LoadingThresholdOutsideRange { value: f64 },
    #[error("the b-spline expansion this rule normalises with: {0}")]
    BasisExpansion(#[from] crate::bspline::BSplineError),
}

/// Eigenvalues descending, each with its eigenvector.
///
/// The sign of an eigenvector is arbitrary in the mathematics and not in a report: two runs
/// of the same data may otherwise hand back loadings that are negatives of each other and
/// read as opposite findings. The element of largest magnitude is made positive, so a
/// component means the same thing every time it is computed.
#[derive(Debug, Clone, PartialEq)]
pub struct Eigendecomposition {
    pub eigenvalues: Vec<f64>,
    /// `eigenvectors[component][element]`, one row per component, ordered with
    /// `eigenvalues`.
    pub eigenvectors: Vec<Vec<f64>>,
}

impl Eigendecomposition {
    /// The share of total variance each component carries, in the same order.
    pub fn variance_shares(&self) -> Vec<f64> {
        let total = compensated_sum(&self.eigenvalues);
        if total <= 0.0 || !total.is_finite() {
            return vec![0.0; self.eigenvalues.len()];
        }
        self.eigenvalues.iter().map(|value| value / total).collect()
    }

    /// How many leading components it takes to reach a stated share of the variance.
    ///
    /// At least one, so a rule that asks for a share smaller than the first component still
    /// gets the component it would have reported.
    pub fn components_for_variance_share(&self, retained_pct: f64) -> Result<usize, WaveformError> {
        if !(retained_pct > 0.0 && retained_pct <= 100.0) {
            return Err(WaveformError::VarianceShareOutsideRange {
                value: retained_pct,
            });
        }
        let mut cumulative = 0.0;
        for (index, share) in self.variance_shares().iter().enumerate() {
            cumulative += share * 100.0;
            if cumulative >= retained_pct {
                return Ok(index + 1);
            }
        }
        Ok(self.eigenvalues.len().max(1))
    }
}

const JACOBI_SWEEP_LIMIT: usize = 100;

/// Eigenvalues and eigenvectors of a symmetric matrix, by cyclic Jacobi rotation.
///
/// Jacobi is chosen over a tridiagonal reduction because it stays accurate on the small
/// eigenvalues, and the trailing components are exactly what a variance-share cutoff is
/// deciding about.
pub fn symmetric_eigendecomposition(
    matrix: &[Vec<f64>],
) -> Result<Eigendecomposition, WaveformError> {
    let width = matrix.len();
    if width == 0 {
        return Ok(Eigendecomposition {
            eigenvalues: Vec::new(),
            eigenvectors: Vec::new(),
        });
    }
    let mut working: Vec<Vec<f64>> = matrix.to_vec();
    let mut rotations: Vec<Vec<f64>> = (0..width)
        .map(|row| (0..width).map(|column| f64::from(row == column)).collect())
        .collect();

    let scale: f64 = matrix
        .iter()
        .flat_map(|row| row.iter())
        .fold(0.0f64, |largest, value| largest.max(value.abs()));
    let tolerance = if scale > 0.0 {
        scale * 1e-15
    } else {
        return Ok(Eigendecomposition {
            eigenvalues: vec![0.0; width],
            eigenvectors: identity_rows(width),
        });
    };

    let mut converged = false;
    for _ in 0..JACOBI_SWEEP_LIMIT {
        let mut off_diagonal = 0.0f64;
        for (row, values) in working.iter().enumerate() {
            for value in values.iter().skip(row + 1) {
                off_diagonal = off_diagonal.max(value.abs());
            }
        }
        if off_diagonal <= tolerance {
            converged = true;
            break;
        }
        for pivot in 0..width {
            for other in (pivot + 1)..width {
                if working[pivot][other].abs() <= tolerance {
                    continue;
                }
                let (cosine, sine) = rotation_zeroing(
                    working[pivot][pivot],
                    working[other][other],
                    working[pivot][other],
                );
                apply_rotation(&mut working, &mut rotations, pivot, other, cosine, sine);
            }
        }
    }
    if !converged {
        return Err(WaveformError::DecompositionDidNotConverge {
            sweeps: JACOBI_SWEEP_LIMIT,
        });
    }

    // Columns of the accumulated rotation are the eigenvectors, so they transpose into rows
    // to sit beside the eigenvalue each belongs to.
    let mut paired: Vec<(f64, Vec<f64>)> = (0..width)
        .map(|component| {
            let vector: Vec<f64> = (0..width)
                .map(|element| rotations[element][component])
                .collect();
            (working[component][component], vector)
        })
        .collect();
    paired.sort_by(|left, right| {
        right
            .0
            .partial_cmp(&left.0)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut eigenvalues = Vec::with_capacity(width);
    let mut eigenvectors = Vec::with_capacity(width);
    for (value, mut vector) in paired {
        fix_sign(&mut vector);
        eigenvalues.push(value);
        eigenvectors.push(vector);
    }
    Ok(Eigendecomposition {
        eigenvalues,
        eigenvectors,
    })
}

fn identity_rows(width: usize) -> Vec<Vec<f64>> {
    (0..width)
        .map(|row| (0..width).map(|column| f64::from(row == column)).collect())
        .collect()
}

/// The rotation that annihilates one off-diagonal element of a symmetric two by two block.
fn rotation_zeroing(diagonal_pivot: f64, diagonal_other: f64, off_diagonal: f64) -> (f64, f64) {
    let theta = (diagonal_other - diagonal_pivot) / (2.0 * off_diagonal);
    // The root of smaller magnitude, written to avoid the cancellation the quadratic
    // formula would suffer when theta is large.
    let tangent = theta.signum() / (theta.abs() + (theta * theta + 1.0).sqrt());
    let cosine = 1.0 / (tangent * tangent + 1.0).sqrt();
    (cosine, tangent * cosine)
}

fn apply_rotation(
    working: &mut [Vec<f64>],
    rotations: &mut [Vec<f64>],
    pivot: usize,
    other: usize,
    cosine: f64,
    sine: f64,
) {
    // Post-multiplication touches two columns of every row.
    for row in working.iter_mut().chain(rotations.iter_mut()) {
        let at_pivot = row[pivot];
        let at_other = row[other];
        row[pivot] = cosine * at_pivot - sine * at_other;
        row[other] = sine * at_pivot + cosine * at_other;
    }
    // Pre-multiplication touches two whole rows, and `pivot` is always the lower index, so
    // the split hands back one of each.
    let (below, from_other) = working.split_at_mut(other);
    for (at_pivot, at_other) in below[pivot].iter_mut().zip(from_other[0].iter_mut()) {
        let held_pivot = *at_pivot;
        let held_other = *at_other;
        *at_pivot = cosine * held_pivot - sine * held_other;
        *at_other = sine * held_pivot + cosine * held_other;
    }
}

fn fix_sign(vector: &mut [f64]) {
    let dominant = vector
        .iter()
        .copied()
        .fold(0.0f64, |largest, value| largest.max(value.abs()));
    let leading = vector.iter().copied().find(|value| value.abs() == dominant);
    if leading.is_some_and(|value| value < 0.0) {
        for value in vector.iter_mut() {
            *value = -*value;
        }
    }
}

/// Every curve's value at every point, checked to be rectangular.
fn checked_curve_set(curves: &[Vec<f64>]) -> Result<usize, WaveformError> {
    if curves.len() < 2 {
        return Err(WaveformError::FewerThanTwoCurves {
            curve_count: curves.len(),
        });
    }
    let point_count = curves[0].len();
    if point_count < 2 {
        return Err(WaveformError::CurveTooShort { point_count });
    }
    for (index, curve) in curves.iter().enumerate() {
        if curve.len() != point_count {
            return Err(WaveformError::RaggedCurveSet {
                index,
                point_count: curve.len(),
                expected: point_count,
            });
        }
    }
    Ok(point_count)
}

/// The mean curve of a set.
pub fn mean_curve(curves: &[Vec<f64>]) -> Result<Vec<f64>, WaveformError> {
    let point_count = checked_curve_set(curves)?;
    let curve_count = curves.len() as f64;
    Ok((0..point_count)
        .map(|point| {
            let column: Vec<f64> = curves.iter().map(|curve| curve[point]).collect();
            compensated_sum(&column) / curve_count
        })
        .collect())
}

/// The variance-covariance matrix across a set of curves, point against point.
///
/// The distinction from a correlation matrix is the rule's own, and it is not cosmetic: a
/// correlation matrix standardises every point to unit variance, which discards the fact
/// that the variance between athletes is many times larger through the propulsive phase
/// than it is during quiet standing. Components extracted from the two matrices describe
/// different things.
pub fn covariance_matrix(curves: &[Vec<f64>]) -> Result<Vec<Vec<f64>>, WaveformError> {
    let point_count = checked_curve_set(curves)?;
    let mean = mean_curve(curves)?;
    let denominator = (curves.len() - 1) as f64;
    let centred: Vec<Vec<f64>> = curves
        .iter()
        .map(|curve| {
            curve
                .iter()
                .zip(&mean)
                .map(|(value, centre)| value - centre)
                .collect()
        })
        .collect();
    Ok((0..point_count)
        .map(|row| {
            (0..point_count)
                .map(|column| {
                    let products: Vec<f64> = centred
                        .iter()
                        .map(|curve| curve[row] * curve[column])
                        .collect();
                    compensated_sum(&products) / denominator
                })
                .collect()
        })
        .collect())
}

/// Whether the loadings are weighted by how much of each element the retained components
/// explain before the rotation runs.
///
/// The two give different rotations and neither is the other's approximation, so the rule
/// that asked for a varimax states which it meant rather than inheriting whichever this
/// function preferred.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VarimaxNormalisation {
    Kaiser,
    Raw,
}

const VARIMAX_SWEEP_LIMIT: usize = 200;
const VARIMAX_ANGLE_TOLERANCE: f64 = 1e-12;

/// Varimax rotation of a loadings matrix, by successive pairwise rotations.
///
/// `loadings[element][component]`. The criterion maximises the variance of the squared
/// loadings within each component, which concentrates each component onto a contiguous
/// stretch of the curve, and it is that concentration that lets a component be read as a
/// phase rather than as a weighting of the whole trace.
pub fn varimax(
    loadings: &[Vec<f64>],
    normalisation: VarimaxNormalisation,
) -> Result<Vec<Vec<f64>>, WaveformError> {
    let element_count = loadings.len();
    if element_count == 0 {
        return Ok(Vec::new());
    }
    let component_count = loadings[0].len();
    if component_count < 2 {
        return Ok(loadings.to_vec());
    }

    let communality: Vec<f64> = loadings
        .iter()
        .map(|row| {
            let squares: Vec<f64> = row.iter().map(|value| value * value).collect();
            compensated_sum(&squares).sqrt()
        })
        .collect();
    let mut rotated: Vec<Vec<f64>> = match normalisation {
        VarimaxNormalisation::Raw => loadings.to_vec(),
        VarimaxNormalisation::Kaiser => loadings
            .iter()
            .zip(&communality)
            .map(|(row, scale)| {
                if *scale > 0.0 {
                    row.iter().map(|value| value / scale).collect()
                } else {
                    row.clone()
                }
            })
            .collect(),
    };

    let elements = element_count as f64;
    for _ in 0..VARIMAX_SWEEP_LIMIT {
        let mut largest_angle = 0.0f64;
        for first in 0..component_count {
            for second in (first + 1)..component_count {
                let mut sum_u = 0.0;
                let mut sum_v = 0.0;
                let mut sum_squares = 0.0;
                let mut sum_cross = 0.0;
                for row in rotated.iter() {
                    let left = row[first];
                    let right = row[second];
                    let u = left * left - right * right;
                    let v = 2.0 * left * right;
                    sum_u += u;
                    sum_v += v;
                    sum_squares += u * u - v * v;
                    sum_cross += 2.0 * u * v;
                }
                let numerator = sum_cross - 2.0 * sum_u * sum_v / elements;
                let denominator = sum_squares - (sum_u * sum_u - sum_v * sum_v) / elements;
                let angle = numerator.atan2(denominator) / 4.0;
                if angle.abs() <= VARIMAX_ANGLE_TOLERANCE {
                    continue;
                }
                largest_angle = largest_angle.max(angle.abs());
                let (sine, cosine) = angle.sin_cos();
                for row in rotated.iter_mut() {
                    let left = row[first];
                    let right = row[second];
                    row[first] = cosine * left + sine * right;
                    row[second] = -sine * left + cosine * right;
                }
            }
        }
        if largest_angle <= VARIMAX_ANGLE_TOLERANCE {
            break;
        }
    }

    if normalisation == VarimaxNormalisation::Kaiser {
        for (row, scale) in rotated.iter_mut().zip(&communality) {
            for value in row.iter_mut() {
                *value *= scale;
            }
        }
    }
    Ok(rotated)
}

/// Each curve expanded in a b-spline basis, with what the expansion cost.
///
/// The basis size and the penalty are settings that move the answer, so they are fields on
/// the result rather than arguments a caller can forget it supplied. A component read off a
/// 20-function basis is not the same component as one read off a 60-function basis.
#[derive(Debug, Clone, PartialEq)]
pub struct BasisExpansion {
    /// `coefficients[curve][function]`.
    pub coefficients: Vec<Vec<f64>>,
    /// Each curve as the basis reconstructs it, on the point grid it came in on.
    pub smoothed: Vec<Vec<f64>>,
    pub basis_size: usize,
    pub basis_degree: usize,
    pub penalty_weight: f64,
    /// How many free parameters the fit spent per curve, which is the number that means
    /// something to a reader where the penalty weight does not.
    pub effective_degrees_of_freedom: Vec<f64>,
}

/// Expand a curve set in a common b-spline basis.
///
/// This is the normalisation the characterising-phase rule names in place of resampling
/// onto a percentage grid, and the one the functional rule expands into before it
/// decomposes anything.
pub fn expand_in_basis(
    curves: &[Vec<f64>],
    basis_size: usize,
    basis_degree: usize,
    penalty_weight: f64,
) -> Result<BasisExpansion, WaveformError> {
    checked_curve_set(curves)?;
    let basis = crate::bspline::Basis::clamped_uniform(basis_size, basis_degree)?;
    let mut coefficients = Vec::with_capacity(curves.len());
    let mut smoothed = Vec::with_capacity(curves.len());
    let mut effective_degrees_of_freedom = Vec::with_capacity(curves.len());
    for curve in curves {
        let fit = basis.fit_penalised(curve, penalty_weight)?;
        coefficients.push(fit.coefficients);
        smoothed.push(fit.fitted);
        effective_degrees_of_freedom.push(fit.effective_degrees_of_freedom);
    }
    Ok(BasisExpansion {
        coefficients,
        smoothed,
        basis_size,
        basis_degree,
        penalty_weight,
        effective_degrees_of_freedom,
    })
}

/// A component-based reduction of a curve set, with what produced it.
#[derive(Debug, Clone, PartialEq)]
pub struct ComponentModel {
    /// `components[component][element]`, in whatever space the rule decomposed.
    pub components: Vec<Vec<f64>>,
    /// The share of total variance each retained component carries, after any rotation.
    pub variance_shares: Vec<f64>,
    /// `scores[curve][component]`, the projection of each centred curve onto each
    /// component.
    pub scores: Vec<Vec<f64>>,
    pub mean: Vec<f64>,
    pub expansion: BasisExpansion,
}

fn centred_rows(rows: &[Vec<f64>], mean: &[f64]) -> Vec<Vec<f64>> {
    rows.iter()
        .map(|row| {
            row.iter()
                .zip(mean)
                .map(|(value, centre)| value - centre)
                .collect()
        })
        .collect()
}

fn project(centred: &[Vec<f64>], components: &[Vec<f64>]) -> Vec<Vec<f64>> {
    centred
        .iter()
        .map(|row| {
            components
                .iter()
                .map(|component| {
                    let terms: Vec<f64> = row
                        .iter()
                        .zip(component)
                        .map(|(value, loading)| value * loading)
                        .collect();
                    compensated_sum(&terms)
                })
                .collect()
        })
        .collect()
}

/// The share of variance each component carries, taken from the loadings themselves.
///
/// After a rotation the eigenvalues no longer describe the components, because a rotation
/// redistributes variance between them while preserving the total. Summing squared loadings
/// gives the share that survives the rotation.
fn shares_from_loadings(components: &[Vec<f64>], total_variance: f64) -> Vec<f64> {
    if total_variance <= 0.0 || !total_variance.is_finite() {
        return vec![0.0; components.len()];
    }
    components
        .iter()
        .map(|component| {
            let squares: Vec<f64> = component.iter().map(|value| value * value).collect();
            compensated_sum(&squares) / total_variance
        })
        .collect()
}

/// Functional principal components of a curve set, decomposed in the basis rather than on
/// the sample grid.
///
/// The decomposition runs on the coefficient matrix, which is what makes this a functional
/// method rather than a point-wise one: two curves that differ only by sampling density
/// give the same coefficients and therefore the same scores.
pub fn functional_principal_components(
    curves: &[Vec<f64>],
    basis_size: usize,
    basis_degree: usize,
    penalty_weight: f64,
    variance_retained_pct: f64,
) -> Result<ComponentModel, WaveformError> {
    let expansion = expand_in_basis(curves, basis_size, basis_degree, penalty_weight)?;
    let mean = mean_curve(&expansion.coefficients)?;
    let covariance = covariance_matrix(&expansion.coefficients)?;
    let decomposition = symmetric_eigendecomposition(&covariance)?;
    let retained = decomposition.components_for_variance_share(variance_retained_pct)?;
    let components: Vec<Vec<f64>> = decomposition
        .eigenvectors
        .into_iter()
        .take(retained)
        .collect();
    let total: f64 = compensated_sum(&decomposition.eigenvalues);
    let centred = centred_rows(&expansion.coefficients, &mean);
    Ok(ComponentModel {
        variance_shares: decomposition
            .eigenvalues
            .iter()
            .take(retained)
            .map(|value| if total > 0.0 { value / total } else { 0.0 })
            .collect(),
        scores: project(&centred, &components),
        components,
        mean,
        expansion,
    })
}

/// One characterising phase: the stretch of the curve a rotated component loads on.
#[derive(Debug, Clone, PartialEq)]
pub struct CharacterisingPhase {
    pub first_index: usize,
    pub last_index: usize,
    /// Where the component loads hardest, which is the instant the phase is named for.
    pub peak_loading_index: usize,
    pub variance_share: f64,
    pub loadings: Vec<f64>,
}

/// Named phases with their variance share, and each curve's value inside each phase.
#[derive(Debug, Clone, PartialEq)]
pub struct CharacterisingPhases {
    pub phases: Vec<CharacterisingPhase>,
    pub mean: Vec<f64>,
    /// `magnitudes[curve][phase]`, each curve at that phase's peak-loading instant.
    pub magnitudes: Vec<Vec<f64>>,
    /// `scores[curve][phase]`, the projection onto the rotated component, which carries
    /// magnitude and time together rather than either alone.
    pub scores: Vec<Vec<f64>>,
    pub variance_retained_pct: f64,
    pub phase_loading_threshold: f64,
    pub normalisation: VarimaxNormalisation,
    pub expansion: BasisExpansion,
}

/// Characterising phases of a curve set, by b-spline normalisation then rotated components.
///
/// The phase boundary is where a component's loading falls below a stated share of its own
/// peak, and that share is an argument because no value for it travels with the rule.
#[allow(clippy::too_many_arguments)]
pub fn characterising_phases(
    curves: &[Vec<f64>],
    basis_size: usize,
    basis_degree: usize,
    penalty_weight: f64,
    variance_retained_pct: f64,
    phase_loading_threshold: f64,
    normalisation: VarimaxNormalisation,
) -> Result<CharacterisingPhases, WaveformError> {
    if !(phase_loading_threshold > 0.0 && phase_loading_threshold <= 1.0) {
        return Err(WaveformError::LoadingThresholdOutsideRange {
            value: phase_loading_threshold,
        });
    }
    let expansion = expand_in_basis(curves, basis_size, basis_degree, penalty_weight)?;
    let mean = mean_curve(&expansion.smoothed)?;
    let covariance = covariance_matrix(&expansion.smoothed)?;
    let decomposition = symmetric_eigendecomposition(&covariance)?;
    let retained = decomposition.components_for_variance_share(variance_retained_pct)?;
    let total: f64 = compensated_sum(&decomposition.eigenvalues);

    // Varimax wants loadings as element by component, and the decomposition hands back
    // component by element.
    let point_count = mean.len();
    let unrotated: Vec<Vec<f64>> = (0..point_count)
        .map(|point| {
            (0..retained)
                .map(|component| {
                    decomposition.eigenvectors[component][point]
                        * decomposition.eigenvalues[component].max(0.0).sqrt()
                })
                .collect()
        })
        .collect();
    let rotated = varimax(&unrotated, normalisation)?;
    let mut components: Vec<Vec<f64>> = (0..retained)
        .map(|component| {
            (0..point_count)
                .map(|point| rotated[point][component])
                .collect()
        })
        .collect();
    for component in components.iter_mut() {
        fix_sign(component);
    }

    let shares = shares_from_loadings(&components, total);
    let phases: Vec<CharacterisingPhase> = components
        .iter()
        .zip(&shares)
        .map(|(loadings, share)| {
            let peak = loadings
                .iter()
                .enumerate()
                .max_by(|left, right| {
                    left.1
                        .abs()
                        .partial_cmp(&right.1.abs())
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .map(|(index, _)| index)
                .unwrap_or(0);
            let floor = loadings[peak].abs() * phase_loading_threshold;
            let mut first = peak;
            while first > 0 && loadings[first - 1].abs() >= floor {
                first -= 1;
            }
            let mut last = peak;
            while last + 1 < loadings.len() && loadings[last + 1].abs() >= floor {
                last += 1;
            }
            CharacterisingPhase {
                first_index: first,
                last_index: last,
                peak_loading_index: peak,
                variance_share: *share,
                loadings: loadings.clone(),
            }
        })
        .collect();

    let centred = centred_rows(&expansion.smoothed, &mean);
    let magnitudes = expansion
        .smoothed
        .iter()
        .map(|curve| {
            phases
                .iter()
                .map(|phase| curve[phase.peak_loading_index])
                .collect()
        })
        .collect();
    Ok(CharacterisingPhases {
        scores: project(&centred, &components),
        phases,
        mean,
        magnitudes,
        variance_retained_pct,
        phase_loading_threshold,
        normalisation,
        expansion,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn multiply(left: &[Vec<f64>], right: &[Vec<f64>]) -> Vec<Vec<f64>> {
        let inner = right.len();
        (0..left.len())
            .map(|row| {
                (0..right[0].len())
                    .map(|column| {
                        let terms: Vec<f64> = (0..inner)
                            .map(|k| left[row][k] * right[k][column])
                            .collect();
                        compensated_sum(&terms)
                    })
                    .collect()
            })
            .collect()
    }

    #[test]
    fn a_decomposition_reconstructs_the_matrix_it_came_from() {
        let matrix = vec![
            vec![4.0, 1.0, -2.0, 2.0],
            vec![1.0, 2.0, 0.0, 1.0],
            vec![-2.0, 0.0, 3.0, -2.0],
            vec![2.0, 1.0, -2.0, -1.0],
        ];
        let decomposition = symmetric_eigendecomposition(&matrix).unwrap();

        // V diag(lambda) V^T is the original matrix when the decomposition is right, and
        // nothing weaker distinguishes a correct answer from a plausible one.
        let vectors_as_columns: Vec<Vec<f64>> = (0..4)
            .map(|element| {
                (0..4)
                    .map(|component| decomposition.eigenvectors[component][element])
                    .collect()
            })
            .collect();
        let scaled: Vec<Vec<f64>> = vectors_as_columns
            .iter()
            .map(|row| {
                row.iter()
                    .zip(&decomposition.eigenvalues)
                    .map(|(value, eigenvalue)| value * eigenvalue)
                    .collect()
            })
            .collect();
        let transposed: Vec<Vec<f64>> = (0..4)
            .map(|row| {
                (0..4)
                    .map(|column| vectors_as_columns[column][row])
                    .collect()
            })
            .collect();
        let reconstructed = multiply(&scaled, &transposed);
        for row in 0..4 {
            for column in 0..4 {
                assert!(
                    (reconstructed[row][column] - matrix[row][column]).abs() < 1e-10,
                    "element {row},{column} came back {} rather than {}",
                    reconstructed[row][column],
                    matrix[row][column]
                );
            }
        }
    }

    #[test]
    fn eigenvalues_come_back_descending_and_eigenvectors_are_orthonormal() {
        let matrix = vec![
            vec![6.0, 2.0, 1.0],
            vec![2.0, 5.0, 3.0],
            vec![1.0, 3.0, 7.0],
        ];
        let decomposition = symmetric_eigendecomposition(&matrix).unwrap();
        for pair in decomposition.eigenvalues.windows(2) {
            assert!(pair[0] >= pair[1], "eigenvalues are not descending");
        }
        for (first, left) in decomposition.eigenvectors.iter().enumerate() {
            let norm: Vec<f64> = left.iter().map(|value| value * value).collect();
            assert!((compensated_sum(&norm) - 1.0).abs() < 1e-12);
            for right in decomposition.eigenvectors.iter().skip(first + 1) {
                let products: Vec<f64> = left.iter().zip(right).map(|(a, b)| a * b).collect();
                assert!(compensated_sum(&products).abs() < 1e-10);
            }
        }
    }

    #[test]
    fn a_diagonal_matrix_decomposes_to_itself() {
        let matrix = vec![
            vec![3.0, 0.0, 0.0],
            vec![0.0, 9.0, 0.0],
            vec![0.0, 0.0, 1.0],
        ];
        let decomposition = symmetric_eigendecomposition(&matrix).unwrap();
        assert_eq!(decomposition.eigenvalues, vec![9.0, 3.0, 1.0]);
        assert_eq!(decomposition.eigenvectors[0], vec![0.0, 1.0, 0.0]);
    }

    #[test]
    fn the_sign_convention_survives_a_negated_input_vector() {
        // Two matrices whose eigenvectors differ only in sign must hand back the same
        // loadings, or one run reads as the opposite finding of the other.
        let mut vector = vec![-0.8, 0.1, -0.2];
        fix_sign(&mut vector);
        assert!(vector[0] > 0.0);
        let mut already_positive = vec![0.8, -0.1, 0.2];
        let expected = already_positive.clone();
        fix_sign(&mut already_positive);
        assert_eq!(already_positive, expected);
    }

    #[test]
    fn every_component_the_decomposition_returns_obeys_the_sign_convention() {
        let matrices = vec![
            vec![
                vec![3.0, -1.0, 0.0],
                vec![-1.0, 3.0, 0.0],
                vec![0.0, 0.0, 8.0],
            ],
            vec![vec![2.0, -3.0], vec![-3.0, 5.0]],
            vec![
                vec![5.0, 4.0, 1.0],
                vec![4.0, 5.0, 1.0],
                vec![1.0, 1.0, 2.0],
            ],
        ];
        for matrix in &matrices {
            for vector in symmetric_eigendecomposition(matrix).unwrap().eigenvectors {
                let dominant = vector
                    .iter()
                    .copied()
                    .fold(0.0f64, |largest, value| largest.max(value.abs()));
                let leading = vector
                    .iter()
                    .copied()
                    .find(|v| v.abs() == dominant)
                    .unwrap();
                assert!(leading >= 0.0, "{vector:?} leads negative");
            }
        }

        // The control. Unconstrained rotation returns the first of these components as
        // [-r, r, 0], measured, so without the convention the loop above has nothing to
        // catch and passes on every matrix in the list.
        let root_half = 0.5f64.sqrt();
        let flipped = &symmetric_eigendecomposition(&matrices[0])
            .unwrap()
            .eigenvectors[1];
        assert!((flipped[0] - root_half).abs() < 1e-12, "{flipped:?}");
        assert!((flipped[1] + root_half).abs() < 1e-12, "{flipped:?}");
    }

    #[test]
    fn a_variance_share_takes_the_components_that_reach_it() {
        let decomposition = Eigendecomposition {
            eigenvalues: vec![7.0, 2.0, 1.0],
            eigenvectors: vec![vec![1.0], vec![1.0], vec![1.0]],
        };
        assert_eq!(
            decomposition.components_for_variance_share(70.0).unwrap(),
            1
        );
        assert_eq!(
            decomposition.components_for_variance_share(80.0).unwrap(),
            2
        );
        assert_eq!(
            decomposition.components_for_variance_share(99.0).unwrap(),
            3
        );
        assert_eq!(
            decomposition.components_for_variance_share(100.0).unwrap(),
            3
        );
        assert!(decomposition.components_for_variance_share(0.0).is_err());
        assert!(decomposition.components_for_variance_share(101.0).is_err());
    }

    #[test]
    fn a_covariance_matrix_is_symmetric_and_carries_the_variance_on_its_diagonal() {
        let curves = vec![
            vec![1.0, 2.0, 3.0],
            vec![2.0, 4.0, 7.0],
            vec![3.0, 6.0, 11.0],
        ];
        let matrix = covariance_matrix(&curves).unwrap();
        for (row, values) in matrix.iter().enumerate() {
            for (column, value) in values.iter().enumerate() {
                assert!((value - matrix[column][row]).abs() < 1e-15);
            }
        }
        // Sample variance of the first point across the three curves, n minus one.
        assert!((matrix[0][0] - 1.0).abs() < 1e-12);
        assert!((matrix[1][1] - 4.0).abs() < 1e-12);
    }

    #[test]
    fn a_correlation_matrix_would_have_given_different_components() {
        // The rule names the covariance matrix, so the two must be demonstrably different
        // here rather than assumed to be. Point three varies sixteen times as much as
        // point one, and standardising that away moves the leading component.
        let curves = vec![
            vec![1.0, 1.0, 4.0],
            vec![2.0, 2.0, 8.0],
            vec![3.0, 3.0, 12.0],
            vec![4.0, 4.0, 16.0],
        ];
        let covariance = covariance_matrix(&curves).unwrap();
        let correlation: Vec<Vec<f64>> = covariance
            .iter()
            .enumerate()
            .map(|(row, values)| {
                values
                    .iter()
                    .enumerate()
                    .map(|(column, value)| {
                        value / (covariance[row][row] * covariance[column][column]).sqrt()
                    })
                    .collect()
            })
            .collect();
        let from_covariance = symmetric_eigendecomposition(&covariance).unwrap();
        let from_correlation = symmetric_eigendecomposition(&correlation).unwrap();
        let difference: f64 = from_covariance.eigenvectors[0]
            .iter()
            .zip(&from_correlation.eigenvectors[0])
            .map(|(left, right)| (left - right).abs())
            .fold(0.0, f64::max);
        assert!(
            difference > 0.1,
            "the two matrices gave the same leading component, so this test proves nothing"
        );
    }

    #[test]
    fn varimax_leaves_an_already_simple_structure_alone() {
        let loadings = vec![
            vec![1.0, 0.0],
            vec![1.0, 0.0],
            vec![0.0, 1.0],
            vec![0.0, 1.0],
        ];
        let rotated = varimax(&loadings, VarimaxNormalisation::Raw).unwrap();
        for (before, after) in loadings.iter().zip(&rotated) {
            for (left, right) in before.iter().zip(after) {
                assert!(
                    (left - right).abs() < 1e-8,
                    "a simple structure was rotated"
                );
            }
        }
    }

    #[test]
    fn varimax_raises_the_simplicity_criterion_on_a_mixed_structure() {
        let loadings = vec![
            vec![0.707, 0.707],
            vec![0.707, 0.707],
            vec![0.707, -0.707],
            vec![0.707, -0.707],
        ];
        let criterion = |matrix: &[Vec<f64>]| -> f64 {
            let elements = matrix.len() as f64;
            (0..matrix[0].len())
                .map(|component| {
                    let squares: Vec<f64> = matrix
                        .iter()
                        .map(|row| row[component] * row[component])
                        .collect();
                    let fourth: Vec<f64> = squares.iter().map(|value| value * value).collect();
                    let total = compensated_sum(&squares);
                    compensated_sum(&fourth) / elements - (total / elements).powi(2)
                })
                .sum()
        };
        let rotated = varimax(&loadings, VarimaxNormalisation::Raw).unwrap();
        assert!(
            criterion(&rotated) > criterion(&loadings) + 1e-9,
            "the rotation did not raise the criterion it exists to maximise"
        );
    }

    #[test]
    fn kaiser_and_raw_normalisation_are_not_the_same_rotation() {
        // Two rows of very unequal communality, which is the case the normalisation exists
        // for and the case where a silent default would decide the answer.
        let loadings = vec![
            vec![0.9, 0.3],
            vec![0.1, 0.05],
            vec![0.4, -0.8],
            vec![0.2, 0.1],
        ];
        let raw = varimax(&loadings, VarimaxNormalisation::Raw).unwrap();
        let kaiser = varimax(&loadings, VarimaxNormalisation::Kaiser).unwrap();
        let difference: f64 = raw
            .iter()
            .flatten()
            .zip(kaiser.iter().flatten())
            .map(|(left, right)| (left - right).abs())
            .fold(0.0, f64::max);
        assert!(
            difference > 1e-6,
            "the two normalisations agreed, so choosing between them would not matter"
        );
    }

    #[test]
    fn a_ragged_curve_set_is_named_rather_than_truncated() {
        let curves = vec![vec![1.0, 2.0, 3.0], vec![1.0, 2.0]];
        let error = covariance_matrix(&curves).unwrap_err();
        assert!(matches!(
            error,
            WaveformError::RaggedCurveSet { index: 1, .. }
        ));
    }

    /// Curves built from two bumps whose heights vary independently, so the covariance has
    /// two directions to find and a phase rule has something to separate.
    fn correlated_bump_curves() -> Vec<Vec<f64>> {
        let bump = |centre: f64, position: f64| (-((position - centre) / 6.0).powi(2)).exp();
        let shared = [-1.0, -1.0, -0.5, -0.5, 0.5, 0.5, 1.0, 1.0];
        let apart = [1.0, -1.0, 1.0, -1.0, 1.0, -1.0, 1.0, -1.0];
        shared.iter().zip(apart.iter()).map(|(common, split)| {
            let early = 1.0 + 0.6 * common + 0.15 * split;
            let late = 1.0 + 0.6 * common - 0.15 * split;
            (0..60).map(|point| {
                let position = point as f64;
                early * bump(15.0, position) + late * bump(45.0, position)
            }).collect()
        }).collect()
    }

    fn two_bump_curves() -> Vec<Vec<f64>> {
        let bump = |centre: f64, position: f64| (-((position - centre) / 6.0).powi(2)).exp();
        // A factorial over the two heights. Amplitudes that traded off against each other
        // would put all the variance in one direction and give one phase for two bumps.
        [
            (0.5, 0.5),
            (1.5, 0.5),
            (0.5, 1.5),
            (1.5, 1.5),
            (1.0, 0.5),
            (0.5, 1.0),
            (1.5, 1.0),
            (1.0, 1.5),
        ]
        .iter()
        .map(|(early, late)| {
            (0..60)
                .map(|point| {
                    let position = point as f64;
                    early * bump(15.0, position) + late * bump(45.0, position)
                })
                .collect()
        })
        .collect()
    }

    /// Three bumps whose heights vary by clearly decreasing amounts, so the components come
    /// out in a known order of size and a share cutoff has something to cut between.
    fn graded_variance_curves() -> Vec<Vec<f64>> {
        let bump = |centre: f64, position: f64| (-((position - centre) / 5.0).powi(2)).exp();
        let design = [
            (1.0, 1.0, 1.0),
            (1.0, 1.0, -1.0),
            (1.0, -1.0, 1.0),
            (1.0, -1.0, -1.0),
            (-1.0, 1.0, 1.0),
            (-1.0, 1.0, -1.0),
            (-1.0, -1.0, 1.0),
            (-1.0, -1.0, -1.0),
        ];
        design
            .iter()
            .map(|(first, second, third)| {
                (0..60)
                    .map(|point| {
                        let position = point as f64;
                        (1.0 + 0.6 * first) * bump(12.0, position)
                            + (1.0 + 0.2 * second) * bump(30.0, position)
                            + (1.0 + 0.05 * third) * bump(48.0, position)
                    })
                    .collect()
            })
            .collect()
    }

    #[test]
    fn an_expansion_carries_the_basis_and_penalty_that_produced_it() {
        let expansion = expand_in_basis(&two_bump_curves(), 14, 3, 0.5).unwrap();
        assert_eq!(expansion.basis_size, 14);
        assert_eq!(expansion.basis_degree, 3);
        assert_eq!(expansion.penalty_weight, 0.5);
        assert_eq!(expansion.effective_degrees_of_freedom.len(), 8);
        assert_eq!(expansion.smoothed[0].len(), 60);
    }

    #[test]
    fn a_heavier_penalty_spends_fewer_degrees_of_freedom() {
        let curves = two_bump_curves();
        let light = expand_in_basis(&curves, 14, 3, 0.0).unwrap();
        let heavy = expand_in_basis(&curves, 14, 3, 100.0).unwrap();
        assert!(
            heavy.effective_degrees_of_freedom[0] < light.effective_degrees_of_freedom[0],
            "the penalty bought nothing: {} against {}",
            heavy.effective_degrees_of_freedom[0],
            light.effective_degrees_of_freedom[0]
        );
    }

    #[test]
    fn the_variance_share_decides_how_many_components_come_back() {
        let curves = graded_variance_curves();
        let narrow = functional_principal_components(&curves, 14, 3, 0.0, 60.0).unwrap();
        let wide = functional_principal_components(&curves, 14, 3, 0.0, 99.5).unwrap();
        assert!(
            wide.components.len() > narrow.components.len(),
            "the retained share changed nothing: {} against {}",
            wide.components.len(),
            narrow.components.len()
        );
        assert_eq!(narrow.scores.len(), curves.len());
        assert_eq!(narrow.scores[0].len(), narrow.components.len());
    }

    #[test]
    fn the_basis_size_moves_the_components_it_is_required_to_report() {
        let curves = two_bump_curves();
        let small = functional_principal_components(&curves, 8, 3, 0.0, 95.0).unwrap();
        let large = functional_principal_components(&curves, 20, 3, 0.0, 95.0).unwrap();
        assert_eq!(small.expansion.basis_size, 8);
        assert_eq!(large.expansion.basis_size, 20);
        assert_ne!(
            small.components[0].len(),
            large.components[0].len(),
            "the basis size did not reach the decomposition"
        );
    }

    #[test]
    fn the_rotation_separates_components_that_would_otherwise_share_a_bump() {
        let phases = characterising_phases(
            &correlated_bump_curves(),
            14,
            3,
            0.0,
            95.0,
            0.5,
            VarimaxNormalisation::Raw,
        )
        .unwrap();
        assert!(phases.phases.len() >= 2, "one phase for two bumps");
        let first = phases.phases[0].peak_loading_index;
        let second = phases.phases[1].peak_loading_index;
        // The bump heights are correlated, so the unrotated components are the sum and the
        // difference directions and both peak on the first bump, measured at index 15 and
        // 15. The rotation is what puts one component on each bump.
        assert!(
            first.abs_diff(second) > 15,
            "both phases landed on one bump, at {first} and {second}"
        );
        for phase in phases.phases.iter().take(2) {
            assert!(phase.first_index <= phase.peak_loading_index);
            assert!(phase.peak_loading_index <= phase.last_index);
        }
        assert_eq!(phases.magnitudes.len(), 8);
        assert_eq!(phases.magnitudes[0].len(), phases.phases.len());
    }

    #[test]
    fn the_loading_threshold_moves_the_phase_boundary() {
        let curves = two_bump_curves();
        let run = |threshold: f64| {
            characterising_phases(
                &curves,
                14,
                3,
                0.0,
                95.0,
                threshold,
                VarimaxNormalisation::Raw,
            )
            .unwrap()
        };
        let permissive = run(0.1);
        let strict = run(0.9);
        let width =
            |set: &CharacterisingPhases| set.phases[0].last_index - set.phases[0].first_index;
        assert!(
            width(&permissive) > width(&strict),
            "the threshold changed no boundary: {} against {}",
            width(&permissive),
            width(&strict)
        );
        assert_eq!(strict.phase_loading_threshold, 0.9);
    }

    #[test]
    fn a_threshold_outside_its_range_is_refused_rather_than_clamped() {
        let curves = two_bump_curves();
        for threshold in [0.0, -0.2, 1.5] {
            let error = characterising_phases(
                &curves,
                14,
                3,
                0.0,
                95.0,
                threshold,
                VarimaxNormalisation::Raw,
            )
            .unwrap_err();
            assert!(matches!(
                error,
                WaveformError::LoadingThresholdOutsideRange { .. }
            ));
        }
    }

    #[test]
    fn a_basis_larger_than_the_data_is_named_rather_than_guessed_at() {
        let curves = vec![vec![1.0, 2.0, 3.0, 4.0], vec![2.0, 3.0, 4.0, 5.0]];
        let error = expand_in_basis(&curves, 12, 3, 0.0).unwrap_err();
        assert!(matches!(error, WaveformError::BasisExpansion(_)));
    }
}
