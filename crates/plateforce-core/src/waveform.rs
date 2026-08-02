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
    #[error("the residual field has no variation to estimate a smoothness from")]
    FieldSmoothnessUnavailable,
    #[error("no height controls the error rate at alpha = {alpha} over a field this rough")]
    ThresholdOutOfReach { alpha: f64 },
    #[error("a permutation null needs at least one permutation")]
    NoPermutations,
    #[error("{curve_count} curves were offered against {landmark_set_count} landmark sets")]
    LandmarkSetSizeMismatch {
        curve_count: usize,
        landmark_set_count: usize,
    },
    #[error("registration needs at least one designated landmark to carry")]
    NoDesignatedLandmarks,
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

/// Natural log of the gamma function, by the Lanczos series at g = 7.
///
/// Needed because the continuum threshold is a tail probability of Student's t, and there
/// is no gamma function in the standard library.
fn ln_gamma(value: f64) -> f64 {
    const COEFFICIENTS: [f64; 9] = [
        0.9999999999998099,
        676.5203681218851,
        -1259.1392167224028,
        771.3234287776531,
        -176.6150291621406,
        12.507343278686905,
        -0.13857109526572012,
        9.984369578019572e-6,
        1.5056327351493116e-7,
    ];
    if value < 0.5 {
        // Reflection, so the series is only ever evaluated where it converges well.
        (std::f64::consts::PI / (std::f64::consts::PI * value).sin()).ln() - ln_gamma(1.0 - value)
    } else {
        let shifted = value - 1.0;
        let mut series = COEFFICIENTS[0];
        for (index, coefficient) in COEFFICIENTS.iter().enumerate().skip(1) {
            series += coefficient / (shifted + index as f64);
        }
        let t = shifted + 7.5;
        0.5 * (2.0 * std::f64::consts::PI).ln() + (shifted + 0.5) * t.ln() - t + series.ln()
    }
}

/// Continued fraction for the incomplete beta function, by the modified Lentz method.
fn beta_continued_fraction(position: f64, first: f64, second: f64) -> f64 {
    const TINY: f64 = 1e-30;
    let sum = first + second;
    let mut c = 1.0;
    let mut d = 1.0 - sum * position / (first + 1.0);
    if d.abs() < TINY {
        d = TINY;
    }
    d = 1.0 / d;
    let mut result = d;
    for step in 1..300 {
        let index = step as f64;
        let even = index * (second - index) * position
            / ((first + 2.0 * index - 1.0) * (first + 2.0 * index));
        d = 1.0 + even * d;
        if d.abs() < TINY {
            d = TINY;
        }
        c = 1.0 + even / c;
        if c.abs() < TINY {
            c = TINY;
        }
        d = 1.0 / d;
        result *= d * c;
        let odd = -(first + index) * (sum + index) * position
            / ((first + 2.0 * index) * (first + 2.0 * index + 1.0));
        d = 1.0 + odd * d;
        if d.abs() < TINY {
            d = TINY;
        }
        c = 1.0 + odd / c;
        if c.abs() < TINY {
            c = TINY;
        }
        d = 1.0 / d;
        let step_factor = d * c;
        result *= step_factor;
        if (step_factor - 1.0).abs() < 1e-14 {
            break;
        }
    }
    result
}

/// Regularised incomplete beta.
fn regularised_incomplete_beta(position: f64, first: f64, second: f64) -> f64 {
    if position <= 0.0 {
        return 0.0;
    }
    if position >= 1.0 {
        return 1.0;
    }
    let front = (ln_gamma(first + second) - ln_gamma(first) - ln_gamma(second)
        + first * position.ln()
        + second * (1.0 - position).ln())
    .exp();
    if position < (first + 1.0) / (first + second + 2.0) {
        front * beta_continued_fraction(position, first, second) / first
    } else {
        1.0 - front * beta_continued_fraction(1.0 - position, second, first) / second
    }
}

/// The probability that Student's t on `degrees_of_freedom` exceeds `value`.
pub fn student_t_upper_tail(value: f64, degrees_of_freedom: f64) -> f64 {
    if degrees_of_freedom <= 0.0 || !value.is_finite() {
        return f64::NAN;
    }
    let half = 0.5
        * regularised_incomplete_beta(
            degrees_of_freedom / (degrees_of_freedom + value * value),
            0.5 * degrees_of_freedom,
            0.5,
        );
    if value >= 0.0 {
        half
    } else {
        1.0 - half
    }
}

/// Which side of the null a continuum test looks at.
///
/// Two-tailed halves the error the threshold is set for, so a one-tailed threshold applied
/// to a two-tailed question is not conservative and the choice does not default here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tails {
    One,
    Two,
}

/// Whose curves are being compared.
#[derive(Debug, Clone, Copy)]
pub enum ContinuumDesign<'a> {
    /// One set against zero, which is the paired difference after subtraction.
    OneSample { curves: &'a [Vec<f64>] },
    /// Two independent sets.
    TwoSample {
        first: &'a [Vec<f64>],
        second: &'a [Vec<f64>],
    },
}

/// The statistic at every point of the continuum, with what it was computed from.
#[derive(Debug, Clone, PartialEq)]
pub struct ContinuumStatistic {
    pub values: Vec<f64>,
    pub degrees_of_freedom: f64,
    /// Each curve less its own group's mean, which is what the smoothness of the field is
    /// estimated from.
    pub residuals: Vec<Vec<f64>>,
}

fn pooled_statistic(design: ContinuumDesign) -> Result<ContinuumStatistic, WaveformError> {
    match design {
        ContinuumDesign::OneSample { curves } => {
            let point_count = checked_curve_set(curves)?;
            let mean = mean_curve(curves)?;
            let count = curves.len() as f64;
            let residuals = centred_rows(curves, &mean);
            let values = (0..point_count)
                .map(|point| {
                    let squares: Vec<f64> = residuals
                        .iter()
                        .map(|row| row[point] * row[point])
                        .collect();
                    let variance = compensated_sum(&squares) / (count - 1.0);
                    let standard_error = (variance / count).sqrt();
                    if standard_error > 0.0 {
                        mean[point] / standard_error
                    } else {
                        0.0
                    }
                })
                .collect();
            Ok(ContinuumStatistic {
                values,
                degrees_of_freedom: count - 1.0,
                residuals,
            })
        }
        ContinuumDesign::TwoSample { first, second } => {
            let point_count = checked_curve_set(first)?;
            let other_points = checked_curve_set(second)?;
            if other_points != point_count {
                return Err(WaveformError::RaggedCurveSet {
                    index: first.len(),
                    point_count: other_points,
                    expected: point_count,
                });
            }
            let first_mean = mean_curve(first)?;
            let second_mean = mean_curve(second)?;
            let first_count = first.len() as f64;
            let second_count = second.len() as f64;
            let mut residuals = centred_rows(first, &first_mean);
            residuals.extend(centred_rows(second, &second_mean));
            let degrees_of_freedom = first_count + second_count - 2.0;
            let values = (0..point_count)
                .map(|point| {
                    let squares: Vec<f64> = residuals
                        .iter()
                        .map(|row| row[point] * row[point])
                        .collect();
                    let pooled = compensated_sum(&squares) / degrees_of_freedom;
                    let standard_error = (pooled * (1.0 / first_count + 1.0 / second_count)).sqrt();
                    if standard_error > 0.0 {
                        (first_mean[point] - second_mean[point]) / standard_error
                    } else {
                        0.0
                    }
                })
                .collect();
            Ok(ContinuumStatistic {
                values,
                degrees_of_freedom,
                residuals,
            })
        }
    }
}

fn gradient(values: &[f64]) -> Vec<f64> {
    let count = values.len();
    if count < 2 {
        return vec![0.0; count];
    }
    let mut slopes = vec![0.0f64; count];
    slopes[0] = values[1] - values[0];
    slopes[count - 1] = values[count - 1] - values[count - 2];
    for index in 1..count - 1 {
        slopes[index] = (values[index + 1] - values[index - 1]) / 2.0;
    }
    slopes
}

/// How smooth the residual field is, as the full width at half maximum of the smoothing
/// kernel that would produce it, in points.
///
/// A rough field has many independent chances to exceed a threshold and a smooth one has
/// few, so this number is what turns a point-wise threshold into one that controls error
/// over the whole curve.
pub fn residual_field_smoothness(residuals: &[Vec<f64>]) -> Option<f64> {
    let point_count = residuals.first()?.len();
    if residuals.len() < 2 || point_count < 2 {
        return None;
    }
    let slopes: Vec<Vec<f64>> = residuals.iter().map(|row| gradient(row)).collect();
    let mut resels_per_point = Vec::with_capacity(point_count);
    for point in 0..point_count {
        let squares: Vec<f64> = residuals
            .iter()
            .map(|row| row[point] * row[point])
            .collect();
        let total = compensated_sum(&squares);
        if total <= 0.0 {
            continue;
        }
        let slope_squares: Vec<f64> = slopes.iter().map(|row| row[point] * row[point]).collect();
        let normalised = compensated_sum(&slope_squares) / total;
        resels_per_point.push((normalised / (4.0 * 2.0f64.ln())).sqrt());
    }
    if resels_per_point.is_empty() {
        return None;
    }
    let mean = compensated_sum(&resels_per_point) / resels_per_point.len() as f64;
    if mean <= 0.0 || !mean.is_finite() {
        return None;
    }
    Some(1.0 / mean)
}

/// Expected Euler characteristic of the excursion set of a t field above `height`.
///
/// The first term is the chance a single point exceeds the height, the second is the chance
/// the field crosses it somewhere along its length, and the length is counted in resels
/// rather than in samples because two samples inside one smoothing kernel are not two
/// chances.
fn expected_euler_characteristic(height: f64, degrees_of_freedom: f64, resel_count: f64) -> f64 {
    let point_term = student_t_upper_tail(height, degrees_of_freedom);
    let crossing_term = (4.0 * 2.0f64.ln()).sqrt() / (2.0 * std::f64::consts::PI)
        * (1.0 + height * height / degrees_of_freedom).powf(-(degrees_of_freedom - 1.0) / 2.0);
    point_term + resel_count * crossing_term
}

/// Where a t field of this smoothness has an `alpha` chance of reaching, anywhere along it.
pub fn random_field_threshold(
    alpha: f64,
    degrees_of_freedom: f64,
    resel_count: f64,
) -> Option<f64> {
    if !(alpha > 0.0 && alpha < 1.0) || degrees_of_freedom <= 1.0 {
        return None;
    }
    let mut low = 0.0f64;
    let mut high = 100.0f64;
    if expected_euler_characteristic(high, degrees_of_freedom, resel_count) > alpha {
        return None;
    }
    for _ in 0..200 {
        let middle = 0.5 * (low + high);
        if expected_euler_characteristic(middle, degrees_of_freedom, resel_count) > alpha {
            low = middle;
        } else {
            high = middle;
        }
    }
    Some(0.5 * (low + high))
}

/// A stretch of the continuum over which the statistic stayed above the threshold.
#[derive(Debug, Clone, PartialEq)]
pub struct SuprathresholdCluster {
    pub first_index: usize,
    pub last_index: usize,
    pub peak_index: usize,
    pub peak_value: f64,
    /// How far the cluster runs, in points. Two clusters of equal peak are not equally
    /// interesting when one lasts a tenth of the movement and the other one sample.
    pub extent_points: usize,
}

fn clusters_above(values: &[f64], threshold: f64, tails: Tails) -> Vec<SuprathresholdCluster> {
    let exceeds = |value: f64| match tails {
        Tails::One => value > threshold,
        Tails::Two => value.abs() > threshold,
    };
    let mut clusters = Vec::new();
    let mut start: Option<usize> = None;
    for index in 0..=values.len() {
        let inside = index < values.len() && exceeds(values[index]);
        match (start, inside) {
            (None, true) => start = Some(index),
            (Some(first), false) => {
                let last = index - 1;
                let (peak_index, peak_value) = (first..=last)
                    .map(|at| (at, values[at]))
                    .max_by(|left, right| {
                        left.1
                            .abs()
                            .partial_cmp(&right.1.abs())
                            .unwrap_or(std::cmp::Ordering::Equal)
                    })
                    .unwrap_or((first, values[first]));
                clusters.push(SuprathresholdCluster {
                    first_index: first,
                    last_index: last,
                    peak_index,
                    peak_value,
                    extent_points: last - first + 1,
                });
                start = None;
            }
            _ => {}
        }
    }
    clusters
}

/// Where the critical height came from.
///
/// Two rules answer the same question by different routes, and a reader cannot tell which
/// one produced a threshold from the number alone.
#[derive(Debug, Clone, PartialEq)]
pub enum ThresholdSource {
    RandomFieldTheory {
        smoothness_full_width_half_maximum_points: f64,
        resel_count: f64,
    },
    Permutation {
        permutation_count: usize,
        /// The permutations are drawn from this seed, so the same data gives the same
        /// threshold on every run and on every machine.
        seed: u64,
    },
}

/// A continuum test: the statistic, the height it had to clear, and where it cleared it.
#[derive(Debug, Clone, PartialEq)]
pub struct ContinuumInference {
    pub statistic: Vec<f64>,
    pub degrees_of_freedom: f64,
    pub critical_threshold: f64,
    pub alpha: f64,
    pub tails: Tails,
    pub clusters: Vec<SuprathresholdCluster>,
    pub threshold_source: ThresholdSource,
}

fn tail_alpha(alpha: f64, tails: Tails) -> f64 {
    match tails {
        Tails::One => alpha,
        Tails::Two => alpha / 2.0,
    }
}

/// Continuum inference with the critical height set from random field theory.
pub fn continuum_inference_random_field(
    design: ContinuumDesign,
    alpha: f64,
    tails: Tails,
) -> Result<ContinuumInference, WaveformError> {
    let statistic = pooled_statistic(design)?;
    let point_count = statistic.values.len();
    let smoothness = residual_field_smoothness(&statistic.residuals)
        .ok_or(WaveformError::FieldSmoothnessUnavailable)?;
    let resel_count = (point_count - 1) as f64 / smoothness;
    let threshold = random_field_threshold(
        tail_alpha(alpha, tails),
        statistic.degrees_of_freedom,
        resel_count,
    )
    .ok_or(WaveformError::ThresholdOutOfReach { alpha })?;
    Ok(ContinuumInference {
        clusters: clusters_above(&statistic.values, threshold, tails),
        statistic: statistic.values,
        degrees_of_freedom: statistic.degrees_of_freedom,
        critical_threshold: threshold,
        alpha,
        tails,
        threshold_source: ThresholdSource::RandomFieldTheory {
            smoothness_full_width_half_maximum_points: smoothness,
            resel_count,
        },
    })
}

/// A counter-based generator, so a permutation set is reproducible from its seed alone.
fn next_random(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut mixed = *state;
    mixed = (mixed ^ (mixed >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    mixed = (mixed ^ (mixed >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    mixed ^ (mixed >> 31)
}

/// The largest value the statistic reached anywhere, which is the quantity a permutation
/// null is built for.
fn field_maximum(values: &[f64], tails: Tails) -> f64 {
    values
        .iter()
        .map(|value| match tails {
            Tails::One => *value,
            Tails::Two => value.abs(),
        })
        .fold(f64::NEG_INFINITY, f64::max)
}

/// Continuum inference with the critical height set by permutation of the labels.
///
/// The parametric route assumes the residual field is smooth and roughly Gaussian. This one
/// assumes only that the labels are exchangeable under the null, which is why it survives
/// the sharp transitions where that first assumption is untested.
pub fn continuum_inference_permutation(
    design: ContinuumDesign,
    alpha: f64,
    tails: Tails,
    permutation_count: usize,
    seed: u64,
) -> Result<ContinuumInference, WaveformError> {
    if permutation_count == 0 {
        return Err(WaveformError::NoPermutations);
    }
    let observed = pooled_statistic(design)?;
    let mut state = seed;
    let mut maxima = Vec::with_capacity(permutation_count);
    for _ in 0..permutation_count {
        let permuted = match design {
            // Exchangeability under the null for one sample is the sign of each curve, not
            // its position, because there is no label to move.
            ContinuumDesign::OneSample { curves } => {
                let flipped: Vec<Vec<f64>> = curves
                    .iter()
                    .map(|curve| {
                        if next_random(&mut state) & 1 == 0 {
                            curve.clone()
                        } else {
                            curve.iter().map(|value| -value).collect()
                        }
                    })
                    .collect();
                pooled_statistic(ContinuumDesign::OneSample { curves: &flipped })?
            }
            ContinuumDesign::TwoSample { first, second } => {
                let mut pool: Vec<Vec<f64>> = first.iter().chain(second).cloned().collect();
                for index in (1..pool.len()).rev() {
                    let swap = (next_random(&mut state) % (index as u64 + 1)) as usize;
                    pool.swap(index, swap);
                }
                let (left, right) = pool.split_at(first.len());
                pooled_statistic(ContinuumDesign::TwoSample {
                    first: left,
                    second: right,
                })?
            }
        };
        maxima.push(field_maximum(&permuted.values, tails));
    }
    maxima.sort_by(|left, right| left.partial_cmp(right).unwrap_or(std::cmp::Ordering::Equal));
    // The (1 - alpha) quantile of the null maxima, taken so the threshold is a value the
    // null actually produced rather than one interpolated between two of them.
    let rank = (((1.0 - tail_alpha(alpha, tails)) * permutation_count as f64).ceil() as usize)
        .clamp(1, permutation_count);
    let threshold = maxima[rank - 1];
    Ok(ContinuumInference {
        clusters: clusters_above(&observed.values, threshold, tails),
        statistic: observed.values,
        degrees_of_freedom: observed.degrees_of_freedom,
        critical_threshold: threshold,
        alpha,
        tails,
        threshold_source: ThresholdSource::Permutation {
            permutation_count,
            seed,
        },
    })
}

/// Where each designated feature sits on one curve.
///
/// `None` is the rule declining on this curve, which is a different state from a landmark
/// at sample zero and is the state this whole rule turns on.
#[derive(Debug, Clone, PartialEq)]
pub struct DesignatedLandmarks {
    pub indices: Vec<Option<usize>>,
}

/// How many curves carried each designated landmark, out of how many were offered.
///
/// The count travels with its denominator because the fraction is the finding: jump force
/// curves are non-, uni- and bimodal across a population, so a landmark set that registers
/// one cohort cleanly can be undefined for a large share of another, and no source in the
/// sweep reports what that share is.
#[derive(Debug, Clone, PartialEq)]
pub struct LandmarkCoverage {
    /// One entry per designated landmark, in the order they were declared.
    pub carrying_curve_count: Vec<usize>,
    pub curve_count: usize,
}

impl LandmarkCoverage {
    /// The share of curves carrying the landmark at `position`, or nothing when no curves
    /// were offered.
    pub fn share_carrying(&self, position: usize) -> Option<f64> {
        if self.curve_count == 0 {
            return None;
        }
        self.carrying_curve_count
            .get(position)
            .map(|count| *count as f64 / self.curve_count as f64)
    }
}

/// What happened to one curve.
#[derive(Debug, Clone, PartialEq)]
pub enum CurveRegistration {
    Registered(Vec<f64>),
    /// The rule placing this landmark declined on this curve, so there is no feature to
    /// carry to a common time.
    LandmarkAbsent {
        landmark: usize,
    },
    /// Two designated landmarks share an instant or run backwards, and no monotone warp
    /// carries both to their common times.
    LandmarksNotInOrder,
}

impl CurveRegistration {
    pub fn samples(&self) -> Option<&[f64]> {
        match self {
            Self::Registered(values) => Some(values),
            _ => None,
        }
    }
}

/// Curves warped onto common landmark times, and what could not be warped.
#[derive(Debug, Clone, PartialEq)]
pub struct LandmarkRegistration {
    /// One entry per input curve, in input order. A curve that could not be registered
    /// stays in this list saying why, because removing it changes the population the
    /// registered set describes without saying so.
    pub curves: Vec<CurveRegistration>,
    pub coverage: LandmarkCoverage,
    /// Where each designated landmark was carried to, as a fraction of the output length.
    pub common_positions: Vec<f64>,
    pub point_count: usize,
}

impl LandmarkRegistration {
    pub fn registered_count(&self) -> usize {
        self.curves
            .iter()
            .filter(|curve| matches!(curve, CurveRegistration::Registered(_)))
            .count()
    }
}

/// Warp each curve by a monotone function so designated features land at common times.
///
/// The common time for a landmark is the mean of where it sits across the curves that carry
/// it, so the target is the cohort's own centre rather than a position chosen here.
pub fn register_to_landmarks(
    curves: &[Vec<f64>],
    landmarks: &[DesignatedLandmarks],
    point_count: usize,
) -> Result<LandmarkRegistration, WaveformError> {
    if curves.len() != landmarks.len() {
        return Err(WaveformError::LandmarkSetSizeMismatch {
            curve_count: curves.len(),
            landmark_set_count: landmarks.len(),
        });
    }
    checked_curve_set(curves)?;
    if point_count < 2 {
        return Err(WaveformError::CurveTooShort { point_count });
    }
    let designated_count = landmarks[0].indices.len();
    if designated_count == 0 {
        return Err(WaveformError::NoDesignatedLandmarks);
    }
    for (index, set) in landmarks.iter().enumerate() {
        if set.indices.len() != designated_count {
            return Err(WaveformError::LandmarkSetSizeMismatch {
                curve_count: index,
                landmark_set_count: set.indices.len(),
            });
        }
    }

    // A landmark no curve carries has no cohort time, and the coverage count beside it is
    // what says so rather than a position invented here.
    let (carrying_curve_count, common_positions): (Vec<usize>, Vec<f64>) = (0..designated_count)
        .map(|position| {
            let seen: Vec<f64> = curves
                .iter()
                .zip(landmarks)
                .filter_map(|(curve, set)| {
                    set.indices[position].map(|index| index as f64 / (curve.len() - 1) as f64)
                })
                .collect();
            let centre = if seen.is_empty() {
                f64::NAN
            } else {
                compensated_sum(&seen) / seen.len() as f64
            };
            (seen.len(), centre)
        })
        .unzip();

    let registered = curves
        .iter()
        .zip(landmarks)
        .map(|(curve, set)| register_one(curve, set, &common_positions, point_count))
        .collect();

    Ok(LandmarkRegistration {
        curves: registered,
        coverage: LandmarkCoverage {
            carrying_curve_count,
            curve_count: curves.len(),
        },
        common_positions,
        point_count,
    })
}

fn register_one(
    curve: &[f64],
    set: &DesignatedLandmarks,
    common_positions: &[f64],
    point_count: usize,
) -> CurveRegistration {
    let last = (curve.len() - 1) as f64;
    let mut source = vec![0.0f64];
    let mut target = vec![0.0f64];
    for (position, index) in set.indices.iter().enumerate() {
        let Some(index) = index else {
            return CurveRegistration::LandmarkAbsent { landmark: position };
        };
        if !common_positions[position].is_finite() {
            return CurveRegistration::LandmarkAbsent { landmark: position };
        }
        source.push(*index as f64 / last);
        target.push(common_positions[position]);
    }
    source.push(1.0);
    target.push(1.0);

    // Monotone in both coordinates is the rule's own word, and a landmark set that is not
    // strictly ordered cannot be honoured by any warp rather than by this one.
    for pair in source.windows(2) {
        if pair[1] <= pair[0] {
            return CurveRegistration::LandmarksNotInOrder;
        }
    }
    for pair in target.windows(2) {
        if pair[1] <= pair[0] {
            return CurveRegistration::LandmarksNotInOrder;
        }
    }

    let Ok(spline) = crate::resample::CubicSpline::through(curve) else {
        return CurveRegistration::LandmarksNotInOrder;
    };
    let warped = (0..point_count)
        .map(|point| {
            let common = point as f64 / (point_count - 1) as f64;
            let at = source_position(common, &source, &target);
            spline.at(at * last)
        })
        .collect();
    CurveRegistration::Registered(warped)
}

/// Where on the original timebase a common time came from, by straight lines between the
/// anchor pairs.
fn source_position(common: f64, source: &[f64], target: &[f64]) -> f64 {
    if common <= target[0] {
        return source[0];
    }
    for window in 1..target.len() {
        if common <= target[window] {
            let span = target[window] - target[window - 1];
            let along = if span > 0.0 {
                (common - target[window - 1]) / span
            } else {
                0.0
            };
            return source[window - 1] + along * (source[window] - source[window - 1]);
        }
    }
    source[source.len() - 1]
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
        shared
            .iter()
            .zip(apart.iter())
            .map(|(common, split)| {
                let early = 1.0 + 0.6 * common + 0.15 * split;
                let late = 1.0 + 0.6 * common - 0.15 * split;
                (0..60)
                    .map(|point| {
                        let position = point as f64;
                        early * bump(15.0, position) + late * bump(45.0, position)
                    })
                    .collect()
            })
            .collect()
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

    /// A curve with a single bump centred at `centre`, on a 101-point grid.
    fn bumped_curve(centre: usize) -> Vec<f64> {
        (0..101)
            .map(|point| (-(((point as f64) - centre as f64) / 8.0).powi(2)).exp())
            .collect()
    }

    fn at(index: usize) -> DesignatedLandmarks {
        DesignatedLandmarks {
            indices: vec![Some(index)],
        }
    }

    #[test]
    fn registration_carries_features_from_different_times_onto_one() {
        let curves = vec![bumped_curve(30), bumped_curve(60)];
        let sets = vec![at(30), at(60)];
        let result = register_to_landmarks(&curves, &sets, 101).unwrap();
        assert_eq!(result.registered_count(), 2);

        // The common time is the cohort's own mean, 0.45, and both bumps must arrive there.
        assert!((result.common_positions[0] - 0.45).abs() < 1e-12);
        let peak_of = |curve: &CurveRegistration| {
            curve
                .samples()
                .unwrap()
                .iter()
                .enumerate()
                .max_by(|left, right| left.1.partial_cmp(right.1).unwrap())
                .unwrap()
                .0
        };
        let first = peak_of(&result.curves[0]);
        let second = peak_of(&result.curves[1]);
        assert!(
            first.abs_diff(second) <= 1,
            "the two bumps landed at {first} and {second}"
        );
        assert!(
            first.abs_diff(45) <= 1,
            "the common time was 0.45 and the bump arrived at {first}"
        );

        // Unregistered, the same two peaks are thirty points apart, so the warp did the
        // work rather than the curves already agreeing.
        assert_eq!(
            curves[0]
                .iter()
                .enumerate()
                .max_by(|l, r| l.1.partial_cmp(r.1).unwrap())
                .unwrap()
                .0,
            30
        );
    }

    #[test]
    fn a_curve_missing_a_landmark_is_reported_with_its_denominator_and_not_dropped() {
        let curves = vec![bumped_curve(30), bumped_curve(60), bumped_curve(45)];
        let sets = vec![
            at(30),
            DesignatedLandmarks {
                indices: vec![None],
            },
            at(45),
        ];
        let result = register_to_landmarks(&curves, &sets, 101).unwrap();

        // Three went in and three come back. A curve that cannot be registered stays in the
        // list saying why, because removing it changes the population the registered set
        // describes without saying so.
        assert_eq!(result.curves.len(), 3);
        assert_eq!(result.registered_count(), 2);
        assert_eq!(
            result.curves[1],
            CurveRegistration::LandmarkAbsent { landmark: 0 }
        );
        assert_eq!(result.coverage.curve_count, 3);
        assert_eq!(result.coverage.carrying_curve_count, vec![2]);
        assert!((result.coverage.share_carrying(0).unwrap() - 2.0 / 3.0).abs() < 1e-12);
    }

    #[test]
    fn landmarks_that_share_an_instant_cannot_be_carried_by_a_monotone_warp() {
        let curves = vec![bumped_curve(30), bumped_curve(60)];
        let sets = vec![
            DesignatedLandmarks {
                indices: vec![Some(30), Some(30)],
            },
            DesignatedLandmarks {
                indices: vec![Some(40), Some(70)],
            },
        ];
        let result = register_to_landmarks(&curves, &sets, 101).unwrap();
        assert_eq!(result.curves[0], CurveRegistration::LandmarksNotInOrder);
        assert!(matches!(result.curves[1], CurveRegistration::Registered(_)));
        // Both curves offered both landmarks, so coverage counts them even though one
        // curve could not be warped. Absence and disorder are different states.
        assert_eq!(result.coverage.carrying_curve_count, vec![2, 2]);
    }

    #[test]
    fn a_landmark_no_curve_carries_leaves_every_curve_unregistered_rather_than_guessed_at() {
        let curves = vec![bumped_curve(30), bumped_curve(60)];
        let sets = vec![
            DesignatedLandmarks {
                indices: vec![Some(30), None],
            },
            DesignatedLandmarks {
                indices: vec![Some(60), None],
            },
        ];
        let result = register_to_landmarks(&curves, &sets, 101).unwrap();
        assert_eq!(result.registered_count(), 0);
        assert_eq!(result.coverage.carrying_curve_count, vec![2, 0]);
        assert_eq!(result.coverage.share_carrying(1).unwrap(), 0.0);
    }

    #[test]
    fn a_registration_with_no_designated_landmarks_is_refused() {
        let curves = vec![bumped_curve(30), bumped_curve(60)];
        let sets = vec![
            DesignatedLandmarks { indices: vec![] },
            DesignatedLandmarks { indices: vec![] },
        ];
        assert!(matches!(
            register_to_landmarks(&curves, &sets, 101).unwrap_err(),
            WaveformError::NoDesignatedLandmarks
        ));
        assert!(matches!(
            register_to_landmarks(&curves, &sets[..1], 101).unwrap_err(),
            WaveformError::LandmarkSetSizeMismatch { .. }
        ));
    }

    #[test]
    fn the_log_gamma_matches_values_that_are_known_in_closed_form() {
        // Gamma(1) = 1, Gamma(1/2) = sqrt(pi), Gamma(5) = 4! = 24, Gamma(10) = 9! = 362880.
        assert!(ln_gamma(1.0).abs() < 1e-12);
        assert!((ln_gamma(0.5) - std::f64::consts::PI.sqrt().ln()).abs() < 1e-12);
        assert!((ln_gamma(5.0) - 24.0f64.ln()).abs() < 1e-11);
        assert!((ln_gamma(10.0) - 362_880.0f64.ln()).abs() < 1e-10);
    }

    #[test]
    fn the_student_tail_matches_the_table_it_replaces() {
        // One degree of freedom is Cauchy, where the tail is 0.5 - atan(t)/pi exactly.
        assert!((student_t_upper_tail(1.0, 1.0) - 0.25).abs() < 1e-12);
        assert!((student_t_upper_tail(0.0, 1.0) - 0.5).abs() < 1e-12);
        // Two-tailed five percent critical values, from the published table.
        assert!((student_t_upper_tail(2.228, 10.0) - 0.025).abs() < 1e-4);
        assert!((student_t_upper_tail(2.042, 30.0) - 0.025).abs() < 1e-4);
        assert!((student_t_upper_tail(1.960, 100_000.0) - 0.025).abs() < 1e-4);
        // Symmetry, which is the property that lets one branch serve both signs.
        for value in [0.3, 1.1, 2.7] {
            let sum = student_t_upper_tail(value, 7.0) + student_t_upper_tail(-value, 7.0);
            assert!((sum - 1.0).abs() < 1e-12);
        }
    }

    #[test]
    fn a_field_with_no_extent_needs_only_the_point_wise_height() {
        // As the field shrinks to a single independent point the correction must vanish,
        // and what is left is the ordinary one-tailed critical value: 1.8125 at ten degrees
        // of freedom and five percent.
        let threshold = random_field_threshold(0.05, 10.0, 0.0).unwrap();
        assert!(
            (threshold - 1.8125).abs() < 1e-3,
            "the uncorrected height came back {threshold}"
        );
    }

    #[test]
    fn a_longer_rougher_field_has_to_clear_a_higher_bar() {
        let short = random_field_threshold(0.05, 20.0, 1.0).unwrap();
        let long = random_field_threshold(0.05, 20.0, 30.0).unwrap();
        assert!(
            long > short + 0.5,
            "extent bought nothing: {long} against {short}"
        );
        // And the correction is the whole reason this rule exists rather than a t test at
        // every point, so it must be well above the point-wise height.
        let point_wise = random_field_threshold(0.05, 20.0, 0.0).unwrap();
        assert!(long > point_wise + 1.0, "{long} against {point_wise}");
    }

    #[test]
    fn a_smooth_field_reads_as_smoother_than_a_rough_one() {
        let smooth: Vec<Vec<f64>> = (0..8)
            .map(|curve| {
                (0..64)
                    .map(|point| {
                        ((point as f64) / 20.0 + curve as f64).sin() * (1.0 + curve as f64 * 0.1)
                    })
                    .collect()
            })
            .collect();
        let rough: Vec<Vec<f64>> = (0..8)
            .map(|curve| {
                (0..64)
                    .map(|point| {
                        let mixed = (point * 7919 + curve * 104_729) as f64;
                        (mixed.sin() * 1_000.0).fract() - 0.5
                    })
                    .collect()
            })
            .collect();
        let smooth_width = residual_field_smoothness(&smooth).unwrap();
        let rough_width = residual_field_smoothness(&rough).unwrap();
        assert!(
            smooth_width > rough_width * 2.0,
            "smoothness did not separate: {smooth_width} against {rough_width}"
        );
    }

    #[test]
    fn clusters_carry_the_stretch_they_ran_over_and_not_only_their_peak() {
        let field = vec![0.0, 3.0, 4.0, 3.5, 0.0, 0.0, 5.0, 0.0];
        let found = clusters_above(&field, 2.0, Tails::One);
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].first_index, 1);
        assert_eq!(found[0].last_index, 3);
        assert_eq!(found[0].extent_points, 3);
        assert_eq!(found[0].peak_index, 2);
        assert_eq!(found[1].extent_points, 1);
        // Two tails read a trough as a finding and one tail does not.
        let signed = vec![0.0, -4.0, 0.0];
        assert_eq!(clusters_above(&signed, 2.0, Tails::One).len(), 0);
        assert_eq!(clusters_above(&signed, 2.0, Tails::Two).len(), 1);
    }

    /// Two groups alike everywhere except points 30 to 40, where the second is lifted.
    fn two_groups_apart_over_a_known_window() -> (Vec<Vec<f64>>, Vec<Vec<f64>>) {
        let shape = |point: usize, wobble: f64| {
            (point as f64 / 12.0).sin() + wobble * (point as f64 / 31.0).cos()
        };
        let first: Vec<Vec<f64>> = (0..10)
            .map(|curve| {
                let wobble = 0.02 * curve as f64;
                (0..64).map(|point| shape(point, wobble)).collect()
            })
            .collect();
        let second: Vec<Vec<f64>> = (0..10)
            .map(|curve| {
                let wobble = 0.02 * curve as f64;
                (0..64)
                    .map(|point| {
                        let lift = if (30..=40).contains(&point) { 2.0 } else { 0.0 };
                        shape(point, wobble) + lift
                    })
                    .collect()
            })
            .collect();
        (first, second)
    }

    #[test]
    fn a_difference_confined_to_one_window_is_reported_in_that_window() {
        let (first, second) = two_groups_apart_over_a_known_window();
        let design = ContinuumDesign::TwoSample {
            first: &first,
            second: &second,
        };
        let result = continuum_inference_random_field(design, 0.05, Tails::Two).unwrap();
        assert!(!result.clusters.is_empty(), "the lift was not found at all");
        let cluster = &result.clusters[0];
        assert!(
            cluster.first_index >= 25 && cluster.last_index <= 45,
            "the cluster ran {}..{} and the lift was 30..40",
            cluster.first_index,
            cluster.last_index
        );
        assert_eq!(result.degrees_of_freedom, 18.0);
        assert!(matches!(
            result.threshold_source,
            ThresholdSource::RandomFieldTheory { .. }
        ));

        // The correction has to have been applied, not merely available. Measured, this
        // field is 9.9 points wide at half maximum and 6.4 resels long, which lifts the
        // height from 2.10 to 3.43. A run that never reached for the smoothness would sit
        // at the point-wise value and every assertion above would still hold.
        let point_wise =
            random_field_threshold(result.alpha / 2.0, result.degrees_of_freedom, 0.0).unwrap();
        assert!(
            result.critical_threshold > point_wise + 1.0,
            "the height was not corrected for extent: {} against {point_wise}",
            result.critical_threshold
        );
        let ThresholdSource::RandomFieldTheory { resel_count, .. } = result.threshold_source else {
            panic!("the recorded source is not the one that ran")
        };
        assert!(
            resel_count > 1.0,
            "the field measured {resel_count} resels long"
        );
    }

    #[test]
    fn two_curve_sets_that_do_not_differ_produce_no_cluster() {
        let (first, _) = two_groups_apart_over_a_known_window();
        let second = first.clone();
        let design = ContinuumDesign::TwoSample {
            first: &first,
            second: &second,
        };
        let result = continuum_inference_random_field(design, 0.05, Tails::Two).unwrap();
        assert!(
            result.clusters.is_empty(),
            "identical sets produced {} clusters",
            result.clusters.len()
        );
    }

    #[test]
    fn a_permutation_threshold_is_the_same_on_every_run_from_one_seed() {
        let (first, second) = two_groups_apart_over_a_known_window();
        let design = ContinuumDesign::TwoSample {
            first: &first,
            second: &second,
        };
        let run = |seed: u64| {
            continuum_inference_permutation(design, 0.05, Tails::Two, 200, seed).unwrap()
        };
        let once = run(20_260_802);
        let again = run(20_260_802);
        assert_eq!(once.critical_threshold, again.critical_threshold);
        assert_eq!(
            once.threshold_source,
            ThresholdSource::Permutation {
                permutation_count: 200,
                seed: 20_260_802,
            }
        );
        assert!(!once.clusters.is_empty(), "the lift was not found");

        // And the seed has to reach the permutations rather than only the record. A run
        // that ignored it would be reproducible too, and would file a seed that did not
        // produce the number beside it.
        let mut heights: Vec<String> = [1u64, 2, 3, 7, 20_260_802]
            .iter()
            .map(|seed| format!("{:.9}", run(*seed).critical_threshold))
            .collect();
        heights.sort();
        heights.dedup();
        assert!(
            heights.len() > 1,
            "every seed gave one height, so the seed reached nothing"
        );
    }

    #[test]
    fn the_two_routes_to_a_threshold_reach_comparable_heights() {
        // They answer the same question by different assumptions, and the entry records
        // that they are reported to agree closely. A gross disagreement here would mean one
        // of the two is wrong rather than that the literature is.
        let (first, second) = two_groups_apart_over_a_known_window();
        let design = ContinuumDesign::TwoSample {
            first: &first,
            second: &second,
        };
        let parametric = continuum_inference_random_field(design, 0.05, Tails::Two).unwrap();
        let permuted = continuum_inference_permutation(design, 0.05, Tails::Two, 500, 7).unwrap();
        let gap = (parametric.critical_threshold - permuted.critical_threshold).abs();
        assert!(
            gap < parametric.critical_threshold,
            "{} against {}",
            parametric.critical_threshold,
            permuted.critical_threshold
        );
    }

    #[test]
    fn a_permutation_run_with_no_permutations_is_refused() {
        let (first, second) = two_groups_apart_over_a_known_window();
        let design = ContinuumDesign::TwoSample {
            first: &first,
            second: &second,
        };
        let error = continuum_inference_permutation(design, 0.05, Tails::Two, 0, 1).unwrap_err();
        assert!(matches!(error, WaveformError::NoPermutations));
    }

    #[test]
    fn a_one_sample_field_takes_its_degrees_of_freedom_from_the_curve_count() {
        let curves: Vec<Vec<f64>> = (0..12)
            .map(|curve| {
                (0..40)
                    .map(|point| 1.0 + (point as f64 / 9.0).sin() + curve as f64 * 0.03)
                    .collect()
            })
            .collect();
        let result = continuum_inference_random_field(
            ContinuumDesign::OneSample { curves: &curves },
            0.05,
            Tails::Two,
        )
        .unwrap();
        assert_eq!(result.degrees_of_freedom, 11.0);
    }

    #[test]
    fn a_basis_larger_than_the_data_is_named_rather_than_guessed_at() {
        let curves = vec![vec![1.0, 2.0, 3.0, 4.0], vec![2.0, 3.0, 4.0, 5.0]];
        let error = expand_in_basis(&curves, 12, 3, 0.0).unwrap_err();
        assert!(matches!(error, WaveformError::BasisExpansion(_)));
    }
}
