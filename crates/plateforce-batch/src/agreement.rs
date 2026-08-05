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
use plateforce_core::{DispersionEstimator, RefusalCode};
use serde::Serialize;

use crate::engine::BatchRequest;
use crate::fingerprint::provenance_id;
use crate::identity::{Session, TrialSet};
use crate::relations::{ProvenanceRow, RefusalRow};

/// The statistic ids this crate can resolve, in one table.
///
/// One place resolves a statistic id, so a rule reports the same id whether it works or
/// declines.
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
    /// The code the rule declined under, beside the sentence it generated. A reader of this
    /// export branches on the code the same way a caller of any other surface does, rather
    /// than matching on the prose.
    pub failure_code: String,
    pub failure_reason: String,
    pub provenance_id: String,
}

impl PairedRow {
    pub fn header() -> Vec<String> {
        [
            "trial_id",
            "subject",
            "variant_label",
            "method_ids",
            "quantity",
            "value",
            "failure_code",
            "failure_reason",
            "provenance_id",
        ]
        .iter()
        .map(|name| name.to_string())
        .collect()
    }

    pub fn cells(&self) -> Vec<String> {
        vec![
            self.trial_id.clone(),
            self.subject.clone(),
            self.variant_label.clone(),
            self.method_ids.join(" "),
            self.quantity.clone(),
            self.value
                .map(crate::relations::format_value)
                .unwrap_or_default(),
            self.failure_code.clone(),
            self.failure_reason.clone(),
            self.provenance_id.clone(),
        ]
    }
}

/// What a compare run produced.
#[derive(Debug, Clone)]
pub struct BatchCompareResult {
    pub paired: Vec<PairedRow>,
    /// One row per distinct chain, keyed the way `analyse` keys its own, so a paired value
    /// reaches the rule that produced it rather than only the rule's name.
    pub provenance: Vec<ProvenanceRow>,
    pub refusals: Vec<RefusalRow>,
    pub quantity: String,
    /// The step the run swept, carried so the record says what was compared rather than only
    /// which rules were named.
    pub slot: String,
    /// The registry construct that step fills. The registry declares `movement_onset` and
    /// `system_weight` and declares no `onset` or `weighing`, so the slot word alone hands a
    /// reader a name they cannot look up.
    pub construct: String,
    pub method_ids: Vec<String>,
    /// Rules this run bound that the sweep did not vary, each with the rule it was pinned to.
    ///
    /// Taken from the sweep's own account of what it held rather than derived a second time
    /// here. Without it a figure taken over one step reads as a figure taken over the run.
    pub held_fixed: Vec<plateforce_analysis::spread::HeldRule>,
    /// What the caller said about the capture, and which saved plate they were told it by.
    pub capture: Option<plateforce_core::Capture>,
    /// How the traces were read, so a reader of the record can read them the same way. The
    /// sample rate above all: these exports carry none, and reading 1200 Hz as 1000 moves
    /// every velocity, displacement, impulse and rate of force development by 20 percent.
    pub format: crate::identity::SourceFormat,
    pub trial_identity: String,
    /// Samples that read as the declared convention writes a measurement nobody took, and
    /// samples that carried no number at all. Two states, counted apart, because a run that
    /// treated one as the other computed system weight from something that is not force.
    pub samples_matching_the_convention: usize,
    pub samples_carrying_no_number: usize,
    /// Trials that produced a value for every method, which is the denominator of any
    /// paired statistic taken over this run.
    pub complete_pairs: usize,
    pub trial_count: usize,
    pub files_found: usize,
    pub files_without_declared_suffix: usize,
    /// Of the files the suffixes kept, the ones the identity could not name.
    pub files_unidentified: usize,
}

impl BatchCompareResult {
    pub fn coverage(&self) -> String {
        format!(
            "{}, {} of {} named, paired {} of {} rows, {} methods x {} trials, failed {} of {}",
            crate::engine::files_line(self.files_found, self.files_without_declared_suffix),
            self.trial_count,
            self.files_found,
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
    /// The published code for this fault. Every refusal the crate emits answers to one of
    /// these, so a caller reading a batch row and a caller reading a compare result meet the
    /// same word for the same failure.
    pub fn code(&self) -> RefusalCode {
        match self {
            // The pattern that would name a subject is the thing left unstated, so the
            // remedy is to state it rather than to repair the data.
            AgreementRefusal::RequiredParametersUnstated { .. }
            | AgreementRefusal::SubjectUnitWithoutGrouping => {
                RefusalCode::RequiredParameterUnstated
            }
            AgreementRefusal::NotTheSameRepetition { .. } => RefusalCode::ObservationsNotPaired,
            AgreementRefusal::ConventionsDiffer { .. } => RefusalCode::ConventionsNotComparable,
            AgreementRefusal::NotEnoughPairs { .. } => RefusalCode::NotEnoughObservations,
        }
    }

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

/// The two enumerations `agreement.bland_altman_loa` publishes in prose, each value beside the
/// word a caller states it by.
///
/// They sit here rather than in the registry because `Parameter.published_values` holds
/// numbers and these are words. When the schema carries enumerated values, these go.
///
/// Paired rather than listed apart from the values they name. A list of words beside a match
/// on those words is two lists, and the words are what a refusal offers a caller: a value this
/// software takes with no word in the list is one nobody can ask for, and a word in the list
/// nothing takes is one the refusal offers and the parse declines.
/// `every_value_these_choices_take_is_a_word_a_caller_can_state` holds each to the other.
///
/// The estimator's own words come from `DispersionEstimator`, which is where they are spelled
/// for every crate that names one. This crate offering a caller a word that surface spells
/// differently would be two vocabularies for one fact.
pub const UNIT_OF_ANALYSIS_VALUES: [(UnitOfAnalysis, &str); 2] = [
    (UnitOfAnalysis::EVERY[0], UnitOfAnalysis::PUBLISHED[0]),
    (UnitOfAnalysis::EVERY[1], UnitOfAnalysis::PUBLISHED[1]),
];
pub const DISPERSION_VALUES: [(DispersionEstimator, &str); 2] = [
    (
        DispersionEstimator::EVERY[0],
        DispersionEstimator::PUBLISHED[0],
    ),
    (
        DispersionEstimator::EVERY[1],
        DispersionEstimator::PUBLISHED[1],
    ),
];

/// The words one of those enumerations offers, which is what a refusal names.
fn words_of<T: Copy>(values: &[(T, &'static str)]) -> Vec<String> {
    values.iter().map(|(_, word)| (*word).to_string()).collect()
}

/// The value a word names, or nothing where the enumeration has no such word.
fn value_of<T: Copy>(values: &[(T, &'static str)], written: &str) -> Option<T> {
    values
        .iter()
        .find(|(_, word)| *word == written)
        .map(|(value, _)| *value)
}

/// Whether a paired statistic is taken over trials or over athletes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnitOfAnalysis {
    Trial,
    Subject,
}

impl UnitOfAnalysis {
    /// Every word this choice publishes, in the same order as `EVERY`.
    pub const PUBLISHED: [&'static str; 2] = ["trial", "subject"];

    /// The published word for this value.
    pub fn as_published_str(self) -> &'static str {
        match self {
            UnitOfAnalysis::Trial => Self::PUBLISHED[0],
            UnitOfAnalysis::Subject => Self::PUBLISHED[1],
        }
    }

    /// Every value this choice takes.
    pub const EVERY: [Self; 2] = [UnitOfAnalysis::Trial, UnitOfAnalysis::Subject];
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
            let mut legal = words_of(&UNIT_OF_ANALYSIS_VALUES);
            legal.extend(words_of(&DISPERSION_VALUES));
            return Err(AgreementRefusal::RequiredParametersUnstated {
                parameters: missing,
                legal,
            });
        }
        let Some(unit) = value_of(
            &UNIT_OF_ANALYSIS_VALUES,
            unit_of_analysis.unwrap_or_default(),
        ) else {
            return Err(AgreementRefusal::RequiredParametersUnstated {
                parameters: vec!["unit_of_analysis".to_string()],
                legal: words_of(&UNIT_OF_ANALYSIS_VALUES),
            });
        };
        let Some(spread) = value_of(&DISPERSION_VALUES, dispersion.unwrap_or_default()) else {
            return Err(AgreementRefusal::RequiredParametersUnstated {
                parameters: vec!["dispersion".to_string()],
                legal: words_of(&DISPERSION_VALUES),
            });
        };
        Ok(Self {
            unit_of_analysis: unit,
            dispersion: spread,
        })
    }
}

/// A compare run: one trace, several methods, paired values out.
///
/// The axis arrives whole rather than as the word the sweep is keyed by and the rules beside
/// it. Taken apart, the construct the axis also carries has no field to arrive on and every
/// caller dropped it, so the record named the step by a word the registry does not declare.
pub struct BatchCompareRequest {
    pub analysis: BatchRequest,
    pub axis: crate::sweep::Axis,
    pub quantity: String,
}

/// Every rule a variant ran under, with the rules the sweep varied put back.
///
/// `Variant::method_ids` is built from the three landmark fields alone, so a sweep over a
/// construct computed from the landmarks returns the same three ids for every variant. Two
/// numbers that differ then carry one chain and one `provenance_id`, which is the record
/// asserting sameness where the values disagree. `settings` is the sweep's own statement of
/// what it varied, so any id in it that the list does not already carry is appended.
///
/// A value axis puts a number in `settings` rather than a rule id, and a number is not a rule,
/// so only entries the binding table answers to are taken.
fn swept_rules_included(mut method_ids: Vec<String>, settings: &[(String, String)]) -> Vec<String> {
    for (_, value) in settings {
        let is_a_rule = plateforce_analysis::BINDINGS
            .iter()
            .any(|binding| binding.id == value);
        if is_a_rule && !method_ids.contains(value) {
            method_ids.push(value.clone());
        }
    }
    method_ids
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
    let mut held_fixed = Vec::new();
    let mut matched_the_convention_total = 0usize;
    let mut carried_no_number_total = 0usize;

    // A file the identity could not name is refused by name here as it is under `analyse`,
    // because a sweep that dropped it would answer how far two rules disagree over a
    // population it never said it had narrowed.
    // A file the identity could not name is refused by name here as it is under `analyse`,
    // because a sweep that dropped it would answer how far two rules disagree over a
    // population it never said it had narrowed.
    for unidentified in &set.unidentified {
        refusals.push(crate::engine::unidentified_row(
            unidentified,
            refusals.len(),
        ));
    }

    for (trial_id, entry) in set.iter() {
        trial_count += 1;
        let subject = entry
            .subject
            .as_ref()
            .map(|key| key.label())
            .unwrap_or_default();

        let trial = match entry.source.read(&set.format) {
            Ok((trial, _, reported)) => {
                // The reader's own account of what it treated as missing, totalled as the
                // analysing run totals it. Discarded here, the record said nothing about a
                // marker read as force, which moves system weight and everything after it.
                matched_the_convention_total += reported.matched_the_convention;
                carried_no_number_total += reported.carried_no_number;
                trial
            }
            Err(error) => {
                refusals.push(RefusalRow {
                    trial_id: trial_id.clone(),
                    ordinal: 0,
                    code: "column_not_found".to_string(),
                    method_id: String::new(),
                    slot: request.axis.slot.clone(),
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
                slot: request.axis.slot.clone(),
                parameter: None,
                values: Vec::new(),
                options: Vec::new(),
                method_ids: request.axis.method_ids.clone(),
            }],
            quantity_key: request.quantity.clone(),
            maximum_combinations: request.axis.method_ids.len().max(1),
        };

        match plateforce_analysis::spread::run(&trial, &sweep) {
            Ok(response) => {
                // The sweep's own statement of what it did not vary, taken from it rather than
                // worked out again here. The base request and the axis are the same on every
                // trial, so the first answer is the run's answer.
                if held_fixed.is_empty() {
                    held_fixed = response.held_fixed;
                }
                for variant in response.variants {
                    paired.push(PairedRow {
                        trial_id: trial_id.clone(),
                        subject: subject.clone(),
                        variant_label: variant.label,
                        method_ids: swept_rules_included(variant.method_ids, &variant.settings),
                        quantity: request.quantity.clone(),
                        value: variant.value,
                        // A variant that failed is listed with its reason and stays in the
                        // denominator, which the sweep already does and this must not undo.
                        failure_code: variant
                            .failure_reason
                            .as_ref()
                            .map(|refusal| refusal.code.wire_name().to_string())
                            .unwrap_or_default(),
                        failure_reason: variant
                            .failure_reason
                            .map(|refusal| refusal.message().to_string())
                            .unwrap_or_default(),
                        provenance_id: String::new(),
                    });
                }
            }
            // The sweep says which code it declined under, so this row carries that rather
            // than a code chosen here for every failure the sweep can have.
            Err(declined) => refusals.push(crate::engine::refusal_row(trial_id, 0, &declined)),
        }
    }

    // A variant names the rules it ran under, so the chain behind each paired value is
    // recorded and keyed rather than left as a label a reader has to interpret.
    //
    // `Variant::method_ids` carries the three landmark fields and nothing else, so two variants
    // differing only in a rule computed from the landmarks arrive here identical. The sweep's
    // own `settings` is the record of what it varied, so the swept rule is taken from there and
    // the chain distinguishes what the numbers distinguish.
    let mut chains: BTreeMap<String, Vec<ProvenanceRow>> = BTreeMap::new();
    for row in &mut paired {
        let rows: Vec<ProvenanceRow> = row
            .method_ids
            .iter()
            .enumerate()
            .map(|(depth, method_id)| ProvenanceRow {
                provenance_id: String::new(),
                quantity: row.quantity.clone(),
                depth,
                method_id: method_id.clone(),
                parameter: String::new(),
                value: String::new(),
                source: "stated".to_string(),
            })
            .collect();
        let identifier = provenance_id(&rows);
        row.provenance_id = identifier.clone();
        chains.entry(identifier.clone()).or_insert_with(|| {
            rows.into_iter()
                .map(|mut entry| {
                    entry.provenance_id = identifier.clone();
                    entry
                })
                .collect()
        });
    }

    let complete_pairs = complete_pairs(&paired, request.axis.method_ids.len());
    BatchCompareResult {
        paired,
        provenance: chains.into_values().flatten().collect(),
        refusals,
        quantity: request.quantity.clone(),
        slot: request.axis.slot.clone(),
        construct: request.axis.construct.clone(),
        method_ids: request.axis.method_ids.clone(),
        held_fixed,
        capture: request.analysis.capture.clone(),
        format: set.format.clone(),
        trial_identity: set.identity.describe(),
        samples_matching_the_convention: matched_the_convention_total,
        samples_carrying_no_number: carried_no_number_total,
        complete_pairs,
        trial_count,
        files_found: set.files_found,
        files_without_declared_suffix: set.files_without_declared_suffix,
        files_unidentified: set.unidentified.len(),
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
    // The same population every other figure over this run is taken over. A trial a gate
    // removed carries its values in the results table, so reading the table without asking
    // which trials the run kept puts a removed trial into a reliability figure.
    let population = result.population();
    Some(
        sessions
            .into_iter()
            .map(|session| {
                session
                    .trial_ids
                    .iter()
                    .filter(|id| population.contains(id))
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
    dispersion.as_published_str()
}

/// One subject's coefficient of variation, re-exported so a caller building the set the mean
/// is taken over reaches the same function rather than writing a second one.
pub use plateforce_core::agreement::coefficient_of_variation as subject_coefficient_of_variation;

/// The record a compare run carries: what it ran over, what it varied, and what identifies it.
///
/// Every fact a reader would otherwise have to fetch from somewhere else is here as the thing
/// itself. The digests identify the run; they do not describe it, and a reader holding a
/// digest and no block cannot say what plate the numbers came off or at what rate they were
/// sampled.
#[derive(Debug, Clone, PartialEq, Serialize, serde::Deserialize)]
pub struct CompareRunRow {
    pub plateforce_version: String,
    /// The revision the caller pinned, and null when they pinned none. Null rather than
    /// absent, as every other record on this surface writes it: a missing key cannot be told
    /// apart from a surface that never carried the field.
    pub registry_version: Option<String>,
    /// The revision the registry names about itself. What the data claims, never what the
    /// caller cited.
    pub registry_declared_version: Option<String>,
    pub registry_digest: Option<String>,
    /// The request the sweep varied, before it was varied: every rule and value this run held
    /// still, plus the pin. Not the identity of what ran, which is this request crossed with
    /// the axis, and two comparisons over one folder that swept different rules carry one
    /// value here. What separates them is `construct`, `method_ids`, `held_fixed` and
    /// `run_fingerprint`, which take the axis in.
    pub base_request_digest: String,
    pub quantity: String,
    /// The step the sweep varied. A record that named the rules but not the step they filled
    /// says which rules ran and not what they were compared as.
    pub slot: String,
    /// The same step as the registry declares it, so a reader looks the construct up rather
    /// than a word that appears in no registry file.
    pub construct: String,
    pub method_ids: Vec<String>,
    /// The rules this run bound and did not vary. The other half of what the figure is over:
    /// a spread across takeoff rules taken under one onset rule is not a spread across the
    /// run, and only this says which onset rule stood still.
    pub held_fixed: Vec<plateforce_analysis::spread::HeldRule>,
    /// The plate and its settings as the caller stated them, carried whole. A trace of forces
    /// says none of it, so a comparison that dropped it published numbers no other lab could
    /// tell apart from their own.
    pub acquisition: plateforce_core::Acquisition,
    pub acquisition_complete: bool,
    /// The saved plate the block was filled from, absent where the caller typed the members
    /// or stated none.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plate_profile: Option<plateforce_core::PlateProfileAttribution>,
    /// How the traces were read: the delimiter, the column, the rate, the missing-sample
    /// convention and which names in the folder were trials.
    pub format: crate::identity::SourceFormat,
    pub trial_identity: String,
    /// Names carrying a declared trial suffix. The denominator the file counts are over.
    pub files_found: usize,
    /// Names the run met carrying none of them, outside `files_found` rather than inside it.
    pub files_without_declared_suffix: usize,
    /// Of those the suffixes kept, the ones the identity could not name.
    pub files_unidentified: usize,
    pub trial_count: usize,
    /// Trials that produced a value for every method, which is the denominator of any paired
    /// statistic taken over this run.
    pub complete_pairs: usize,
    pub paired_rows: usize,
    pub failed_rows: usize,
    pub distinct_provenance_count: usize,
    /// Samples read as the declared convention writes a measurement nobody took, and samples
    /// that carried no number at all, counted apart.
    pub samples_matching_the_convention: usize,
    pub samples_carrying_no_number: usize,
    /// What two labs compare to say they ran the same comparison, and null where the
    /// acquisition block was not filled. Withheld rather than published incomplete, so a run
    /// nobody can declare a match for carries nothing that could be mistaken for one.
    pub run_fingerprint: Option<String>,
}

impl BatchCompareResult {
    /// The run row, measured from what the run actually produced.
    ///
    /// The stamp arrives whole for the reason `SpreadDocument::of` takes it whole, and is
    /// destructured without a rest pattern so a fact added to it is a compile error here
    /// rather than one this record quietly stops carrying. Three same-typed options passed
    /// positionally accept a transposed pair and compile, and this pair has been transposed
    /// before, publishing the registry's own claim under the caller's name.
    pub fn run_row(
        &self,
        registry: &plateforce_core::provenance::RegistryStamp,
        base_request_digest: &str,
    ) -> CompareRunRow {
        let distinct: std::collections::BTreeSet<&str> = self
            .paired
            .iter()
            .map(|row| row.provenance_id.as_str())
            .collect();
        let plateforce_core::provenance::RegistryStamp {
            version: registry_version,
            declared_version: registry_declared_version,
            digest: registry_digest,
        } = registry.clone();
        let plateforce_core::Capture {
            acquisition,
            plate_profile,
        } = self.capture.clone().unwrap_or_default();

        let mut row = CompareRunRow {
            plateforce_version: env!("CARGO_PKG_VERSION").to_string(),
            registry_version,
            registry_declared_version,
            registry_digest,
            base_request_digest: base_request_digest.to_string(),
            quantity: self.quantity.clone(),
            slot: self.slot.clone(),
            construct: self.construct.clone(),
            method_ids: self.method_ids.clone(),
            held_fixed: self.held_fixed.clone(),
            // Read off the block rather than taken from the caller, which is how two surfaces
            // came to publish a literal false beside a block nobody could give them.
            acquisition_complete: acquisition.is_complete(),
            acquisition,
            plate_profile,
            format: self.format.clone(),
            trial_identity: self.trial_identity.clone(),
            files_found: self.files_found,
            files_without_declared_suffix: self.files_without_declared_suffix,
            files_unidentified: self.files_unidentified,
            trial_count: self.trial_count,
            complete_pairs: self.complete_pairs,
            paired_rows: self.paired.len(),
            failed_rows: self.paired.iter().filter(|row| row.value.is_none()).count(),
            distinct_provenance_count: distinct.len(),
            samples_matching_the_convention: self.samples_matching_the_convention,
            samples_carrying_no_number: self.samples_carrying_no_number,
            run_fingerprint: None,
        };
        let ids: std::collections::BTreeSet<String> =
            distinct.iter().map(|id| (*id).to_string()).collect();
        row.run_fingerprint = crate::fingerprint::compare_run_fingerprint(&row, &ids)
            .published()
            .map(str::to_string);
        row
    }

    /// `{"ok": {...}}`, the same envelope shape a single-mode run returns.
    pub fn to_json(
        &self,
        registry: &plateforce_core::provenance::RegistryStamp,
        base_request_digest: &str,
    ) -> String {
        serde_json::json!({
            "ok": {
                "run": self.run_row(registry, base_request_digest),
                "paired": self.paired,
                "provenance": self.provenance,
                "refusals": self.refusals,
            }
        })
        .to_string()
    }

    /// The relations on disk, with the record beside them.
    ///
    /// A compare run answers how far two methods disagree, so a table of paired numbers with
    /// no record of which rules produced which column leaves every number in it
    /// unattributable. The refusal to write one without its record is the same one the trial
    /// writer applies, for the same reason.
    pub fn write_csv(
        &self,
        directory: &std::path::Path,
        registry: &plateforce_core::provenance::RegistryStamp,
        base_request_digest: &str,
    ) -> Result<Vec<std::path::PathBuf>, crate::WriteRefusal> {
        std::fs::create_dir_all(directory).map_err(|source| crate::WriteRefusal::Io {
            path: directory.display().to_string(),
            source,
        })?;
        let write = |name: &str, body: String| -> Result<std::path::PathBuf, crate::WriteRefusal> {
            let path = directory.join(name);
            std::fs::write(&path, body).map_err(|source| crate::WriteRefusal::Io {
                path: path.display().to_string(),
                source,
            })?;
            Ok(path)
        };

        let record = serde_json::to_string_pretty(&self.run_row(registry, base_request_digest))
            .unwrap_or_default();
        Ok(vec![
            write("compare-run.json", record)?,
            write(
                "paired.csv",
                crate::write_csv::render_table(
                    PairedRow::header(),
                    self.paired.iter().map(PairedRow::cells),
                ),
            )?,
            write(
                "provenance.csv",
                crate::write_csv::render_table(
                    crate::relations::ProvenanceRow::header(),
                    self.provenance
                        .iter()
                        .map(crate::relations::ProvenanceRow::cells),
                ),
            )?,
            write(
                "refusals.csv",
                crate::write_csv::render_table(
                    RefusalRow::header(),
                    self.refusals.iter().map(RefusalRow::cells),
                ),
            )?,
        ])
    }
}
