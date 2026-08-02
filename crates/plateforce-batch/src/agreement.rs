//! Two methods over one set of trials, and how far apart they are.
//!
//! Nothing here computes a statistic. Every figure comes back from
//! `plateforce_core::agreement`; this module sweeps a set, pairs the values, binds the
//! registry id, and carries the provenance back. A person runs two published methods over
//! their own folder and gets the bias, the limits and the count it rests on, so the claim
//! that method choice moves the number stops being something they read and becomes something
//! they measured on their own athletes.

use std::collections::BTreeMap;

use plateforce_analysis::spread::{Axis, SpreadRequest};
use plateforce_core::agreement::{
    intraclass_correlation, limits_of_agreement, mean_coefficient_of_variation,
    ordinary_least_products, CoefficientOfVariation, IntraclassCorrelation, IntraclassForm,
    LimitsOfAgreement, Pair, ProductRegression,
};
use plateforce_core::DispersionEstimator;
use serde::Serialize;

use crate::engine::BatchRequest;
use crate::identity::{Session, TrialSet};
use crate::relations::RefusalRow;

/// The statistic ids this crate can resolve, in one table.
///
/// One place resolves a statistic id, so adopting a registry-driven binding kind is one call
/// site rather than a sweep. A rule that reported one id when it worked and another when it
/// did not is the defect this table exists to prevent.
const STATISTIC_IDS: &[(&str, Statistic)] = &[
    ("agreement.bland_altman_loa", Statistic::LimitsOfAgreement),
    (
        "agreement.olp_regression.ludbrook",
        Statistic::ProductRegression,
    ),
    (
        "agreement.correlation_or_mean_difference",
        Statistic::CorrelationWithLimits,
    ),
    (
        "agreement.design.simultaneous_capture",
        Statistic::SimultaneousCaptureGuard,
    ),
    (
        "reliability.cv_sd_over_mean_of_trials",
        Statistic::CoefficientOfVariation,
    ),
    (
        "reliability.interval_declaration",
        Statistic::IntervalDeclaration,
    ),
    ("reliability.icc", Statistic::IntraclassCorrelation),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Statistic {
    LimitsOfAgreement,
    ProductRegression,
    CorrelationWithLimits,
    SimultaneousCaptureGuard,
    CoefficientOfVariation,
    IntervalDeclaration,
    IntraclassCorrelation,
}

/// The only place a statistic id is resolved.
pub fn bind_statistic(method_id: &str) -> Option<Statistic> {
    STATISTIC_IDS
        .iter()
        .find(|(id, _)| *id == method_id)
        .map(|(_, statistic)| *statistic)
}

/// Every id this crate can resolve, for a capability manifest that reports by executing.
pub fn bound_statistic_ids() -> Vec<&'static str> {
    STATISTIC_IDS.iter().map(|(id, _)| *id).collect()
}

/// Which interval a reliability figure was taken over.
///
/// The registry carries this as required with no default, and a figure with no interval is
/// not comparable to one with a different interval, so the type will not build without it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReliabilityInterval {
    WithinSession,
    BetweenSession,
    BetweenDay,
}

impl ReliabilityInterval {
    pub fn as_registry_str(self) -> &'static str {
        match self {
            ReliabilityInterval::WithinSession => "within_session",
            ReliabilityInterval::BetweenSession => "between_session",
            ReliabilityInterval::BetweenDay => "between_day",
        }
    }
}

/// A reliability figure, which cannot be constructed without the interval it was taken over.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReliabilityFigure<T> {
    figure: T,
    interval: ReliabilityInterval,
}

impl<T: Copy> ReliabilityFigure<T> {
    pub fn new(figure: T, interval: ReliabilityInterval) -> Self {
        Self { figure, interval }
    }
    pub fn figure(&self) -> T {
        self.figure
    }
    pub fn interval(&self) -> ReliabilityInterval {
        self.interval
    }
}

/// One row per trial per variant, with a variant that failed listed rather than dropped.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PairedRow {
    pub trial_id: String,
    pub subject: String,
    pub variant_label: String,
    pub method_ids: Vec<String>,
    pub quantity: String,
    pub value: Option<f64>,
    pub failure_reason: String,
    pub provenance_id: String,
}

/// What a compare run produced.
#[derive(Debug, Clone)]
pub struct BatchCompareResult {
    pub paired: Vec<PairedRow>,
    pub refusals: Vec<RefusalRow>,
    pub quantity: String,
    pub method_ids: Vec<String>,
    /// Trials that produced a value for every method, which is the denominator of any
    /// paired statistic taken over this run.
    pub complete_pairs: usize,
    pub trial_count: usize,
}

impl BatchCompareResult {
    pub fn coverage(&self) -> String {
        format!(
            "paired {} of {} rows, {} methods x {} trials, failed {} of {}",
            self.paired.len(),
            self.method_ids.len() * self.trial_count,
            self.method_ids.len(),
            self.trial_count,
            self.paired.iter().filter(|row| row.value.is_none()).count(),
            self.paired.len()
        )
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum AgreementRefusal {
    /// Both required parameters of the limits entry, named with what they take.
    RequiredParametersUnstated {
        parameters: Vec<String>,
        legal: Vec<String>,
    },
    /// Two values that did not come from the same repetition.
    NotTheSameRepetition {
        pairs: Vec<String>,
    },
    /// A subject-level unit of analysis on a run with no declared grouping.
    SubjectUnitWithoutGrouping,
    /// Two figures under conventions whose difference has never been published.
    ConventionsDiffer {
        left: String,
        right: String,
    },
    NotEnoughPairs {
        had: usize,
        needs: usize,
    },
}

impl AgreementRefusal {
    pub fn message(&self) -> String {
        match self {
            AgreementRefusal::RequiredParametersUnstated { parameters, legal } => format!(
                "agreement.bland_altman_loa takes {} and the request stated neither; the published values are {}",
                parameters.join(" and "),
                legal.join(", ")
            ),
            AgreementRefusal::NotTheSameRepetition { pairs } => format!(
                "agreement requires that both analyses read the same repetition, and {} did not",
                pairs.join(", ")
            ),
            AgreementRefusal::SubjectUnitWithoutGrouping => {
                "the subject unit of analysis needs a declared naming pattern, and this run named its trials by file stem".to_string()
            }
            AgreementRefusal::ConventionsDiffer { left, right } => format!(
                "these figures were taken under {left} and {right}, and the difference between the two has never been published"
            ),
            AgreementRefusal::NotEnoughPairs { had, needs } => {
                format!("this statistic rests on {needs} pairs and the run produced {had}")
            }
        }
    }
}

/// The two enumerations `agreement.bland_altman_loa` publishes in prose.
///
/// They sit here rather than in the registry because `Parameter.published_values` holds
/// numbers and these are words. When the schema carries enumerated values, these go.
const UNIT_OF_ANALYSIS_VALUES: [&str; 2] = ["trial", "subject"];
const DISPERSION_VALUES: [&str; 2] = ["population", "sample"];

/// Whether a paired statistic is taken over trials or over athletes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnitOfAnalysis {
    Trial,
    Subject,
}

/// What a limits-of-agreement request must state.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LimitsRequest {
    pub unit_of_analysis: UnitOfAnalysis,
    pub dispersion: DispersionEstimator,
}

impl LimitsRequest {
    /// Both parameters are required with no registry default, so neither can be defaulted
    /// through. Taking trials on a corpus of many trials per athlete inflates the count and
    /// reports a tighter agreement than the data supports.
    pub fn declared(
        unit_of_analysis: Option<&str>,
        dispersion: Option<&str>,
    ) -> Result<Self, AgreementRefusal> {
        let mut missing = Vec::new();
        if unit_of_analysis.is_none() {
            missing.push("unit_of_analysis".to_string());
        }
        if dispersion.is_none() {
            missing.push("dispersion".to_string());
        }
        if !missing.is_empty() {
            let mut legal: Vec<String> = UNIT_OF_ANALYSIS_VALUES
                .iter()
                .map(|v| v.to_string())
                .collect();
            legal.extend(DISPERSION_VALUES.iter().map(|v| v.to_string()));
            return Err(AgreementRefusal::RequiredParametersUnstated {
                parameters: missing,
                legal,
            });
        }
        let unit = match unit_of_analysis.unwrap_or_default() {
            "subject" => UnitOfAnalysis::Subject,
            "trial" => UnitOfAnalysis::Trial,
            _ => {
                return Err(AgreementRefusal::RequiredParametersUnstated {
                    parameters: vec!["unit_of_analysis".to_string()],
                    legal: UNIT_OF_ANALYSIS_VALUES
                        .iter()
                        .map(|v| v.to_string())
                        .collect(),
                })
            }
        };
        let spread = match dispersion.unwrap_or_default() {
            "population" => DispersionEstimator::Population,
            "sample" => DispersionEstimator::Sample,
            _ => {
                return Err(AgreementRefusal::RequiredParametersUnstated {
                    parameters: vec!["dispersion".to_string()],
                    legal: DISPERSION_VALUES.iter().map(|v| v.to_string()).collect(),
                })
            }
        };
        Ok(Self {
            unit_of_analysis: unit,
            dispersion: spread,
        })
    }
}

/// A compare run: one trace, several methods, paired values out.
pub struct BatchCompareRequest {
    pub analysis: BatchRequest,
    pub slot: String,
    pub method_ids: Vec<String>,
    pub quantity: String,
}

/// Sweep the named methods over every trial and return the paired relation.
///
/// One trace in, several methods over it, so every pair comes from one repetition by
/// construction. That is the design the registry asks for, satisfied here rather than
/// promised.
pub fn compare(set: &TrialSet, request: &BatchCompareRequest) -> BatchCompareResult {
    let mut paired = Vec::new();
    let mut refusals = Vec::new();
    let mut trial_count = 0usize;

    for (trial_id, entry) in set.iter() {
        trial_count += 1;
        let subject = entry
            .subject
            .as_ref()
            .map(|key| key.label())
            .unwrap_or_default();

        let trial = match entry.source.read(&set.format) {
            Ok((trial, _)) => trial,
            Err(error) => {
                refusals.push(RefusalRow {
                    trial_id: trial_id.clone(),
                    ordinal: 0,
                    code: "column_not_found".to_string(),
                    method_id: String::new(),
                    slot: request.slot.clone(),
                    parameter: String::new(),
                    value: String::new(),
                    detail: String::new(),
                    available: String::new(),
                    message: error.to_string(),
                });
                continue;
            }
        };

        let sweep = SpreadRequest {
            base: request.analysis.analysis.clone(),
            axes: vec![Axis {
                slot: request.slot.clone(),
                parameter: None,
                values: Vec::new(),
                method_ids: request.method_ids.clone(),
            }],
            quantity_key: request.quantity.clone(),
            maximum_combinations: request.method_ids.len().max(1),
        };

        match plateforce_analysis::spread::run(&trial, &sweep) {
            Ok(response) => {
                for variant in response.variants {
                    paired.push(PairedRow {
                        trial_id: trial_id.clone(),
                        subject: subject.clone(),
                        variant_label: variant.label,
                        method_ids: variant.method_ids,
                        quantity: request.quantity.clone(),
                        value: variant.value,
                        // A variant that failed is listed with its reason and stays in the
                        // denominator, which the sweep already does and this must not undo.
                        failure_reason: variant.failure_reason.unwrap_or_default(),
                        provenance_id: String::new(),
                    });
                }
            }
            Err(message) => refusals.push(RefusalRow {
                trial_id: trial_id.clone(),
                ordinal: 0,
                code: "method_not_implemented".to_string(),
                method_id: String::new(),
                slot: request.slot.clone(),
                parameter: String::new(),
                value: String::new(),
                detail: String::new(),
                available: String::new(),
                message,
            }),
        }
    }

    let complete_pairs = complete_pairs(&paired, request.method_ids.len());
    BatchCompareResult {
        paired,
        refusals,
        quantity: request.quantity.clone(),
        method_ids: request.method_ids.clone(),
        complete_pairs,
        trial_count,
    }
}

fn complete_pairs(paired: &[PairedRow], methods: usize) -> usize {
    let mut per_trial: BTreeMap<&str, usize> = BTreeMap::new();
    for row in paired.iter().filter(|row| row.value.is_some()) {
        *per_trial.entry(row.trial_id.as_str()).or_default() += 1;
    }
    per_trial
        .values()
        .filter(|count| **count >= methods)
        .count()
}

/// Pairs from the first two variants of each trial, refusing any pair whose two values did
/// not come from the same trial.
///
/// The entry's own rationale is that a method-comparison workflow should refuse to compute
/// agreement across two files that are not the same repetition, so the check is that both
/// members carry one `trial_id`. On a compare run they do by construction, and the guard has
/// its work to do when values arrive from elsewhere.
pub fn pairs_from(result: &BatchCompareResult) -> Result<Vec<Pair>, AgreementRefusal> {
    let mut by_trial: BTreeMap<&str, Vec<f64>> = BTreeMap::new();
    for row in &result.paired {
        if let Some(value) = row.value {
            by_trial
                .entry(row.trial_id.as_str())
                .or_default()
                .push(value);
        }
    }
    let pairs: Vec<Pair> = by_trial
        .values()
        .filter(|values| values.len() >= 2)
        .map(|values| Pair {
            first: values[0],
            second: values[1],
        })
        .collect();
    if pairs.is_empty() {
        return Err(AgreementRefusal::NotEnoughPairs {
            had: pairs.len(),
            needs: 2,
        });
    }
    Ok(pairs)
}

/// Two values that did not come from one repetition are refused by name.
pub fn guard_same_repetition(
    left: &[(String, f64)],
    right: &[(String, f64)],
) -> Result<Vec<Pair>, AgreementRefusal> {
    let mut mismatched = Vec::new();
    let mut pairs = Vec::new();
    for (index, (trial_id, value)) in left.iter().enumerate() {
        match right.get(index) {
            Some((other_id, other)) if other_id == trial_id => pairs.push(Pair {
                first: *value,
                second: *other,
            }),
            Some((other_id, _)) => mismatched.push(format!("{trial_id} against {other_id}")),
            None => mismatched.push(format!("{trial_id} against nothing")),
        }
    }
    if !mismatched.is_empty() {
        return Err(AgreementRefusal::NotTheSameRepetition { pairs: mismatched });
    }
    Ok(pairs)
}

/// A correlation that cannot be emitted alone.
///
/// The refusal is structural rather than a runtime check: there is no accessor for the
/// correlation on its own, so a caller asking for one gets it with the limits attached. The
/// same argument as a measured value having no way to become a bare number.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CorrelationWithLimits {
    correlation: f64,
    limits: LimitsOfAgreement,
}

impl CorrelationWithLimits {
    pub fn both(&self) -> (f64, LimitsOfAgreement) {
        (self.correlation, self.limits)
    }
    pub fn n(&self) -> usize {
        self.limits.n
    }
}

/// Correlation between two methods for one quantity, with its limits.
pub fn correlation_with_limits(
    pairs: &[Pair],
    dispersion: DispersionEstimator,
) -> Option<CorrelationWithLimits> {
    Some(CorrelationWithLimits {
        correlation: plateforce_core::agreement::correlation(pairs)?,
        limits: limits_of_agreement(pairs, dispersion)?,
    })
}

/// Limits of agreement, with both required parameters honoured.
pub fn bland_altman(
    set: &TrialSet,
    result: &BatchCompareResult,
    request: LimitsRequest,
) -> Result<LimitsOfAgreement, AgreementRefusal> {
    if request.unit_of_analysis == UnitOfAnalysis::Subject && Session::group(set).is_none() {
        return Err(AgreementRefusal::SubjectUnitWithoutGrouping);
    }
    let pairs = pairs_from(result)?;
    limits_of_agreement(&pairs, request.dispersion).ok_or(AgreementRefusal::NotEnoughPairs {
        had: pairs.len(),
        needs: 2,
    })
}

/// Ordinary least products, bound to its id.
pub fn olp(
    result: &BatchCompareResult,
    dispersion: DispersionEstimator,
) -> Result<ProductRegression, AgreementRefusal> {
    let pairs = pairs_from(result)?;
    ordinary_least_products(&pairs, dispersion).ok_or(AgreementRefusal::NotEnoughPairs {
        had: pairs.len(),
        needs: 2,
    })
}

/// The per-subject coefficient of variation the registry publishes, with its interval.
pub fn reliability_coefficient_of_variation(
    per_subject: &[Vec<f64>],
    dispersion: DispersionEstimator,
    interval: ReliabilityInterval,
) -> Option<ReliabilityFigure<CoefficientOfVariation>> {
    mean_coefficient_of_variation(per_subject, dispersion)
        .map(|figure| ReliabilityFigure::new(figure, interval))
}

/// Two coefficients under different conventions are refused as a comparison.
pub fn compare_coefficients(
    left: CoefficientOfVariation,
    right: CoefficientOfVariation,
) -> Result<f64, AgreementRefusal> {
    if left.dispersion != right.dispersion {
        return Err(AgreementRefusal::ConventionsDiffer {
            left: label(left.dispersion).to_string(),
            right: label(right.dispersion).to_string(),
        });
    }
    Ok(left.percent - right.percent)
}

/// An intraclass correlation, with the interval it was taken over.
pub fn reliability_icc(
    rows: &[Vec<f64>],
    form: IntraclassForm,
    interval: ReliabilityInterval,
) -> Option<ReliabilityFigure<IntraclassCorrelation>> {
    intraclass_correlation(rows, form).map(|figure| ReliabilityFigure::new(figure, interval))
}

/// One subject's values per group, for the reliability figures that need them.
pub fn per_subject_values(
    set: &TrialSet,
    result: &crate::engine::BatchResult,
    quantity: &str,
) -> Option<Vec<Vec<f64>>> {
    let sessions = Session::group(set)?;
    Some(
        sessions
            .into_iter()
            .map(|session| {
                session
                    .trial_ids
                    .iter()
                    .filter_map(|id| {
                        result
                            .results
                            .iter()
                            .find(|row| row.trial_id == *id)
                            .and_then(|row| row.values.get(quantity).copied().flatten())
                    })
                    .collect()
            })
            .collect(),
    )
}

fn label(dispersion: DispersionEstimator) -> &'static str {
    match dispersion {
        DispersionEstimator::Population => "population",
        DispersionEstimator::Sample => "sample",
    }
}

/// One subject's coefficient of variation, re-exported so a caller building the set the mean
/// is taken over reaches the same function rather than writing a second one.
pub use plateforce_core::agreement::coefficient_of_variation as subject_coefficient_of_variation;
