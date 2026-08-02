//! Reducing several trials to one number, as a bound method rather than a convenience.
//!
//! There is no such thing as the mean here. `trial.aggregation` publishes three mutually
//! incompatible rules and none of them is the arithmetic mean of a subject's trials, so a
//! reduction that took a plain mean and bound it to that id would attach a citation, a method
//! id and a chain to a rule nobody published and the user never chose. It would also look
//! right, round correctly and survive review, which is what makes it worse than a wrong
//! number. That is why the enum has three variants and no fourth, why it has no `Default`,
//! and why the refusal is a test rather than a convention.

use std::collections::BTreeMap;

use plateforce_core::statistics::{mean, standard_deviation};
use plateforce_core::DispersionEstimator;
use serde::{Deserialize, Serialize};

use crate::engine::BatchResult;
use crate::fingerprint::provenance_id;
use crate::identity::{Session, TrialSet};
use crate::relations::{AggregateRow, ProvenanceRow};

/// The registry id every per-athlete reduction here is bound to.
pub const TRIAL_AGGREGATION: &str = "trial.aggregation";

/// The three published rules, and no fourth.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AggregationRule {
    BestOfNByPeakForce,
    MeanOfBestThreeOfAtLeastFive,
    MeanOfBestTwo,
}

impl AggregationRule {
    /// Spelled as the registry spells them, because a user looking for one of these strings
    /// should find it in the files.
    pub const PUBLISHED: [&'static str; 3] = [
        "best_of_n_by_peak_force",
        "mean_of_best_three_of_at_least_five",
        "mean_of_best_two",
    ];

    pub fn as_registry_str(self) -> &'static str {
        match self {
            AggregationRule::BestOfNByPeakForce => Self::PUBLISHED[0],
            AggregationRule::MeanOfBestThreeOfAtLeastFive => Self::PUBLISHED[1],
            AggregationRule::MeanOfBestTwo => Self::PUBLISHED[2],
        }
    }

    pub fn parse(name: &str) -> Result<Self, AggregationRefusal> {
        match name {
            "best_of_n_by_peak_force" => Ok(AggregationRule::BestOfNByPeakForce),
            "mean_of_best_three_of_at_least_five" => {
                Ok(AggregationRule::MeanOfBestThreeOfAtLeastFive)
            }
            "mean_of_best_two" => Ok(AggregationRule::MeanOfBestTwo),
            _ => Err(AggregationRefusal::RuleNotStated),
        }
    }

    /// How many trials must be in the group, given the count the caller declared.
    ///
    /// The declared count is the rule's own `n`, so best of five and best of three are two
    /// different requests of the same rule and the group has to be large enough for the one
    /// that was asked for.
    fn trials_required(self, declared: usize) -> usize {
        match self {
            AggregationRule::BestOfNByPeakForce => declared,
            AggregationRule::MeanOfBestThreeOfAtLeastFive => declared.max(5),
            AggregationRule::MeanOfBestTwo => 2,
        }
    }

    /// How many of the ranked trials it reduces to one number.
    fn trials_taken(self) -> usize {
        match self {
            AggregationRule::BestOfNByPeakForce => 1,
            AggregationRule::MeanOfBestThreeOfAtLeastFive => 3,
            AggregationRule::MeanOfBestTwo => 2,
        }
    }

    /// A declared count the rule cannot honour is refused rather than rounded into range.
    fn check_declared(self, declared: usize) -> Result<(), AggregationRefusal> {
        let floor = match self {
            AggregationRule::BestOfNByPeakForce => 1,
            AggregationRule::MeanOfBestThreeOfAtLeastFive => 5,
            AggregationRule::MeanOfBestTwo => 2,
        };
        if declared < floor {
            return Err(AggregationRefusal::CountBelowRule {
                rule: self.as_registry_str().to_string(),
                declared,
                floor,
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum AggregationRefusal {
    RuleNotStated,
    CountNotStated,
    /// The rule ranks on a quantity the analysis did not produce.
    QuantityAbsent {
        rule: String,
        quantity: String,
    },
    /// Fewer trials in the group than the rule requires.
    TooFewTrials {
        rule: String,
        had: usize,
        needs: usize,
        group: String,
    },
    /// The run declared no pattern, so it has no subject to group by.
    NoDeclaredGrouping {
        template_hint: String,
    },
    /// The declared count is below what the rule can honour.
    CountBelowRule {
        rule: String,
        declared: usize,
        floor: usize,
    },
}

impl AggregationRefusal {
    pub fn message(&self) -> String {
        match self {
            AggregationRefusal::RuleNotStated => format!(
                "{TRIAL_AGGREGATION} takes one of {}, and the request named none",
                AggregationRule::PUBLISHED.join(", ")
            ),
            AggregationRefusal::CountNotStated => format!(
                "{TRIAL_AGGREGATION} takes a count of trials, and best of five and best of three are different numbers"
            ),
            AggregationRefusal::QuantityAbsent { rule, quantity } => format!(
                "{rule} ranks trials on {quantity}, which this analysis did not produce"
            ),
            AggregationRefusal::TooFewTrials {
                rule,
                had,
                needs,
                group,
            } => format!("{rule} reduces {needs} trials and {group} has {had}"),
            AggregationRefusal::NoDeclaredGrouping { template_hint } => format!(
                "this run named its trials by file stem, so it has no subject to group by, and a pattern such as {template_hint} would supply one"
            ),
            AggregationRefusal::CountBelowRule {
                rule,
                declared,
                floor,
            } => format!("{rule} reduces at least {floor} trials and the request declared {declared}"),
        }
    }
}

/// Which trials one reduction is taken over.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupKind {
    Subject,
    Session,
    Run,
}

impl GroupKind {
    pub fn as_str(self) -> &'static str {
        match self {
            GroupKind::Subject => "subject",
            GroupKind::Session => "session",
            GroupKind::Run => "run",
        }
    }
}

/// A reduction the caller asked for.
///
/// Binding this puts `aggregated_value` on the requested path, and
/// `aggregation.peak_of_average_vs_average_of_peaks` on that construct forces a decision, so
/// a run that binds aggregation without resolving that fork refuses before reading a trial.
/// That is the intended behaviour rather than an accident of ordering.
#[derive(Debug, Clone, PartialEq)]
pub struct AggregationRequest {
    pub rule: AggregationRule,
    /// Travels with the value in every export and every label.
    pub n: usize,
    pub group_kind: GroupKind,
    pub quantities: Vec<String>,
    pub dispersion: DispersionEstimator,
}

impl AggregationRequest {
    /// Built from what a caller stated, refusing rather than filling anything in.
    pub fn declared(
        rule: Option<&str>,
        n: Option<usize>,
        group_kind: GroupKind,
        quantities: Vec<String>,
        dispersion: DispersionEstimator,
    ) -> Result<Self, AggregationRefusal> {
        let rule = AggregationRule::parse(rule.unwrap_or_default())?;
        let n = n.ok_or(AggregationRefusal::CountNotStated)?;
        rule.check_declared(n)?;
        Ok(Self {
            rule,
            n,
            group_kind,
            quantities,
            dispersion,
        })
    }
}

/// The groups a reduction runs over, or the refusal that says why there are none.
fn groups(
    set: &TrialSet,
    result: &BatchResult,
    kind: GroupKind,
) -> Result<Vec<(String, Vec<String>)>, AggregationRefusal> {
    let computed: Vec<String> = result
        .results
        .iter()
        .filter(|row| row.refusal_code.is_empty())
        .map(|row| row.trial_id.clone())
        .collect();

    match kind {
        GroupKind::Run => Ok(vec![("run".to_string(), computed)]),
        GroupKind::Subject | GroupKind::Session => {
            let sessions = Session::group(set).ok_or(AggregationRefusal::NoDeclaredGrouping {
                template_hint: "AT{subject}_{trial}".to_string(),
            })?;
            Ok(sessions
                .into_iter()
                .map(|session| {
                    let trials = session
                        .trial_ids
                        .into_iter()
                        .filter(|id| computed.contains(id))
                        .collect();
                    (session.key.label(), trials)
                })
                .collect())
        }
    }
}

/// Reduce every group, one row per group per quantity.
pub fn aggregate(
    set: &TrialSet,
    result: &BatchResult,
    request: &AggregationRequest,
) -> Result<(Vec<AggregateRow>, Vec<ProvenanceRow>), AggregationRefusal> {
    let groups = groups(set, result, request.group_kind)?;
    let mut rows = Vec::new();
    let mut chains = Vec::new();

    for (group_key, trial_ids) in groups {
        let needs = request.rule.trials_required(request.n);
        if trial_ids.len() < needs {
            return Err(AggregationRefusal::TooFewTrials {
                rule: request.rule.as_registry_str().to_string(),
                had: trial_ids.len(),
                needs,
                group: group_key,
            });
        }
        for quantity in &request.quantities {
            let mut values: Vec<f64> = trial_ids
                .iter()
                .filter_map(|id| {
                    result
                        .results
                        .iter()
                        .find(|row| row.trial_id == *id)
                        .and_then(|row| row.values.get(quantity).copied().flatten())
                })
                .collect();
            if values.is_empty() {
                continue;
            }

            // Ranking is on the quantity being aggregated, except for the rule that names
            // peak force, which needs a quantity this analysis does not produce.
            if request.rule == AggregationRule::BestOfNByPeakForce
                && !result
                    .quantities
                    .iter()
                    .any(|key| key.contains("peak_force"))
            {
                return Err(AggregationRefusal::QuantityAbsent {
                    rule: request.rule.as_registry_str().to_string(),
                    quantity: "net_peak_force_newtons".to_string(),
                });
            }

            values.sort_by(|left, right| {
                right.partial_cmp(left).unwrap_or(std::cmp::Ordering::Equal)
            });
            let taken = request.rule.trials_taken().min(values.len());
            let reduced = &values[..taken];

            let (row, chain) = row_for(request, &group_key, quantity, reduced);
            rows.push(row);
            chains.extend(chain);
        }
    }
    Ok((rows, chains))
}

/// One reduced value, and the chain that reaches the rule and the count behind it.
fn row_for(
    request: &AggregationRequest,
    group_key: &str,
    quantity: &str,
    reduced: &[f64],
) -> (AggregateRow, Vec<ProvenanceRow>) {
    let value = mean(reduced);
    let dispersion = standard_deviation(reduced, request.dispersion);

    // A reduction across the subjects in a lab section is a descriptive statistic of the
    // user's own set and no registry entry publishes a rule for it, so the row carries no
    // method id and its chain says the estimator was assumed rather than published.
    let method_id = match request.group_kind {
        GroupKind::Run => String::new(),
        _ => TRIAL_AGGREGATION.to_string(),
    };
    let source = match request.group_kind {
        GroupKind::Run => "assumed",
        _ => "stated",
    };

    let mut chain = vec![
        ProvenanceRow {
            provenance_id: String::new(),
            quantity: quantity.to_string(),
            depth: 0,
            method_id: method_id.clone(),
            parameter: "rule".to_string(),
            value: request.rule.as_registry_str().to_string(),
            source: source.to_string(),
        },
        ProvenanceRow {
            provenance_id: String::new(),
            quantity: quantity.to_string(),
            depth: 0,
            method_id: method_id.clone(),
            parameter: "n".to_string(),
            value: reduced.len().to_string(),
            source: source.to_string(),
        },
    ];
    chain.push(ProvenanceRow {
        provenance_id: String::new(),
        quantity: quantity.to_string(),
        depth: 0,
        method_id: method_id.clone(),
        parameter: "dispersion".to_string(),
        value: dispersion_label(request.dispersion).to_string(),
        source: "assumed".to_string(),
    });

    let identifier = provenance_id(&chain);
    for row in &mut chain {
        row.provenance_id = identifier.clone();
    }

    (
        AggregateRow {
            group_key: group_key.to_string(),
            group_kind: request.group_kind.as_str().to_string(),
            quantity: quantity.to_string(),
            value,
            dispersion,
            n: reduced.len(),
            method_id,
            provenance_id: identifier,
        },
        chain,
    )
}

fn dispersion_label(dispersion: DispersionEstimator) -> &'static str {
    match dispersion {
        DispersionEstimator::Population => "population",
        DispersionEstimator::Sample => "sample",
    }
}

/// The reductions joined onto a result, so `results` gains nothing and loses nothing.
pub fn with_aggregates(
    mut result: BatchResult,
    set: &TrialSet,
    request: &AggregationRequest,
) -> Result<BatchResult, AggregationRefusal> {
    let (rows, chains) = aggregate(set, &result, request)?;
    let mut seen: BTreeMap<String, ()> = BTreeMap::new();
    for chain in chains {
        if seen.insert(chain.provenance_id.clone(), ()).is_none()
            || !result.provenance.contains(&chain)
        {
            result.provenance.push(chain);
        }
    }
    result.aggregates = rows;
    Ok(result)
}
