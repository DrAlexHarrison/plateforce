//! Reducing several trials to one number, as a bound method rather than a convenience.
//!
//! There is no such thing as the mean here. `trial.aggregation` publishes three mutually
//! incompatible rules and none of them is the arithmetic mean of a subject's trials, so a
//! reduction that took a plain mean and bound it to that id would attach a citation, a method
//! id and a chain to a rule nobody published and the user never chose. It would also look
//! right, round correctly and survive review, which is what makes it worse than a wrong
//! number. That is why the enum has three variants and no fourth, why it has no `Default`,
//! and why the refusal is a test rather than a convention.

use std::collections::{BTreeMap, BTreeSet};

use plateforce_analysis::{records_under, BINDINGS};
use plateforce_core::statistics::{mean, standard_deviation};
use plateforce_core::DispersionEstimator;
use serde::{Deserialize, Serialize};

use crate::engine::BatchResult;
use crate::fingerprint::provenance_id;
use crate::identity::{Session, TrialSet};
use crate::relations::{AggregateRow, ProvenanceRow};

/// The registry id every per-athlete reduction here is bound to.
pub const TRIAL_AGGREGATION: &str = "trial.aggregation";

/// The construct the rule whose name carries its own criterion ranks on.
pub const PEAK_FORCE_RANKING_CONSTRUCT: &str =
    plateforce_analysis::slots::net_peak_force::CONSTRUCT;

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

    /// Naming nothing and naming something unpublished are two different mistakes, and a
    /// refusal that reads "the request named none" to a caller who wrote `arithmetic_mean`
    /// tells them nothing about the word they used. Caught 2026-08-05, the first time a surface
    /// could reach this rule at all.
    pub fn parse(name: &str) -> Result<Self, AggregationRefusal> {
        match name {
            "best_of_n_by_peak_force" => Ok(AggregationRule::BestOfNByPeakForce),
            "mean_of_best_three_of_at_least_five" => {
                Ok(AggregationRule::MeanOfBestThreeOfAtLeastFive)
            }
            "mean_of_best_two" => Ok(AggregationRule::MeanOfBestTwo),
            "" => Err(AggregationRefusal::RuleNotStated),
            named => Err(AggregationRefusal::RuleNotPublished {
                named: named.to_string(),
            }),
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
    /// A word the caller wrote that no published rule answers to. It is quoted back, because a
    /// caller cannot correct a word the refusal does not repeat.
    RuleNotPublished {
        named: String,
    },
    CountNotStated,
    /// A rule using the word `best` without carrying its own criterion, where the caller left
    /// the criterion open too.
    RankedByNotStated {
        rule: String,
        /// The constructs this run's results offer, filled by `against` where the caller holds
        /// the folder. Empty where the refusal was raised before a trial was read.
        carried: Vec<String>,
    },
    /// A criterion stated beside a rule whose name already fixes another one.
    RankedByContradictsRule {
        rule: String,
        named: String,
        required: String,
    },
    /// No result column carries a value of the construct the rule ranks on.
    RankingConstructAbsent {
        rule: String,
        construct: String,
        /// The constructs this run's result columns do carry, which are the answers that would
        /// have worked. A caller cannot reach a construct id from anywhere else on this
        /// surface, so a refusal that named only the rejected one leaves them guessing.
        carried: Vec<String>,
    },
    /// More than one result column carries a value of the named construct.
    RankingConstructAmbiguous {
        rule: String,
        construct: String,
        quantities: Vec<String>,
    },
    /// One trial in the group carries no value for the quantity that would order it.
    RankingValueAbsent {
        rule: String,
        quantity: String,
        trial_id: String,
        group: String,
    },
    /// The criterion cannot choose a complete set because trials on both sides of the cutoff
    /// carry the same value.
    RankingTiedAtBoundary {
        rule: String,
        quantity: String,
        group: String,
        value: f64,
        tied: usize,
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

/// What a refusal about a criterion offers instead, as the sentence both of them end with.
///
/// A run whose columns root at no rankable construct says so, because an empty list read as a
/// reader having chosen wrong when nothing would have worked.
fn offer(carried: &[String]) -> String {
    match carried.split_last() {
        None => {
            ". No column in this folder carries a construct this reduction can rank on".to_string()
        }
        Some((last, [])) => format!(". This folder's columns carry {last}"),
        Some((last, before)) => {
            format!(
                ". This folder's columns carry {} and {last}",
                before.join(", ")
            )
        }
    }
}

impl AggregationRefusal {
    /// The same refusal, carrying what this folder's results offer.
    ///
    /// A criterion is refused before a trial is read, where the folder is not yet in hand, and
    /// named again once it is. Every surface calls this on the way to `message`, so a reader
    /// learns the vocabulary from the first refusal rather than by guessing a construct wrongly
    /// to earn it.
    pub fn against(self, result: &BatchResult) -> Self {
        match self {
            AggregationRefusal::RankedByNotStated { rule, .. } => {
                AggregationRefusal::RankedByNotStated {
                    rule,
                    carried: constructs_carried(result),
                }
            }
            other => other,
        }
    }

    pub fn message(&self) -> String {
        match self {
            AggregationRefusal::RuleNotStated => format!(
                "{TRIAL_AGGREGATION} takes one of {}, and the request named none",
                AggregationRule::PUBLISHED.join(", ")
            ),
            AggregationRefusal::RuleNotPublished { named } => format!(
                "{TRIAL_AGGREGATION} takes one of {}, and the request named {named}. There is no \
                 mean of a subject's trials here: none of the three is one, so a reduction that \
                 took a plain mean would attach a citation to a rule nobody published",
                AggregationRule::PUBLISHED.join(", ")
            ),
            AggregationRefusal::CountNotStated => format!(
                "{TRIAL_AGGREGATION} takes a count of trials, and best of five and best of three are different numbers"
            ),
            AggregationRefusal::RankedByNotStated { rule, carried } => format!(
                "{TRIAL_AGGREGATION} publishes no default for ranked_by under {rule}, so it has \
                 to be stated{}",
                offer(carried)
            ),
            AggregationRefusal::RankedByContradictsRule {
                rule,
                named,
                required,
            } => format!(
                "{rule} ranks trials on {required}, and the request named {named} instead"
            ),
            AggregationRefusal::RankingConstructAbsent {
                rule,
                construct,
                carried,
            } => format!(
                "{rule} ranks trials on {construct}, and none of this folder's result columns \
                 carries that construct's value{}",
                offer(carried)
            ),
            AggregationRefusal::RankingConstructAmbiguous {
                rule,
                construct,
                quantities,
            } => format!(
                "{rule} ranks trials on one value of {construct}, and this folder carries that construct in {}, so the criterion does not identify one value",
                quantities.join(", ")
            ),
            AggregationRefusal::RankingValueAbsent {
                rule,
                quantity,
                trial_id,
                group,
            } => format!(
                "{rule} ranks {group} on {quantity}, and {trial_id} carries no value for it"
            ),
            AggregationRefusal::RankingTiedAtBoundary {
                rule,
                quantity,
                group,
                value,
                tied,
            } => format!(
                "{rule} ranks {group} on {quantity}, and {tied} trials carry {value} at the boundary between trials taken and left out"
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
    /// The construct whose value orders the trials before the rule takes any of them.
    pub ranked_by: String,
    pub group_kind: GroupKind,
    pub quantities: Vec<String>,
    pub dispersion: DispersionEstimator,
}

impl AggregationRequest {
    /// Built from what a caller stated, refusing rather than filling anything in.
    pub fn declared(
        rule: Option<&str>,
        n: Option<usize>,
        ranked_by: Option<&str>,
        group_kind: GroupKind,
        quantities: Vec<String>,
        dispersion: DispersionEstimator,
    ) -> Result<Self, AggregationRefusal> {
        let rule = AggregationRule::parse(rule.unwrap_or_default())?;
        let n = n.ok_or(AggregationRefusal::CountNotStated)?;
        rule.check_declared(n)?;
        let stated = ranked_by.filter(|name| !name.is_empty());
        let ranked_by = match (rule, stated) {
            (AggregationRule::BestOfNByPeakForce, None) => PEAK_FORCE_RANKING_CONSTRUCT.to_string(),
            (AggregationRule::BestOfNByPeakForce, Some(PEAK_FORCE_RANKING_CONSTRUCT)) => {
                PEAK_FORCE_RANKING_CONSTRUCT.to_string()
            }
            (AggregationRule::BestOfNByPeakForce, Some(named)) => {
                return Err(AggregationRefusal::RankedByContradictsRule {
                    rule: rule.as_registry_str().to_string(),
                    named: named.to_string(),
                    required: PEAK_FORCE_RANKING_CONSTRUCT.to_string(),
                })
            }
            (_, Some(named)) => named.to_string(),
            (_, None) => {
                return Err(AggregationRefusal::RankedByNotStated {
                    rule: rule.as_registry_str().to_string(),
                    carried: Vec::new(),
                })
            }
        };
        Ok(Self {
            rule,
            n,
            ranked_by,
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
    let computed = result.population();

    match kind {
        GroupKind::Run => Ok(vec![("run".to_string(), computed)]),
        GroupKind::Subject | GroupKind::Session => {
            let sessions = Session::group(set).ok_or(AggregationRefusal::NoDeclaredGrouping {
                template_hint: "AT{subject}_{trial}".to_string(),
            })?;
            // A session is one subject on one occasion, so grouping by subject pools that
            // subject's occasions and grouping by session keeps them apart. Keying both on
            // the session would report one athlete's Monday and Tuesday as two athletes.
            let mut grouped: BTreeMap<String, Vec<String>> = BTreeMap::new();
            for session in sessions {
                let key = match kind {
                    GroupKind::Session => session.key.label(),
                    _ => session.key.subject.clone(),
                };
                grouped.entry(key).or_default().extend(
                    session
                        .trial_ids
                        .into_iter()
                        .filter(|id| computed.contains(id)),
                );
            }
            Ok(grouped.into_iter().collect())
        }
    }
}

/// Reduce every group, one row per group per quantity.
pub fn aggregate(
    set: &TrialSet,
    result: &BatchResult,
    request: &AggregationRequest,
) -> Result<(Vec<AggregateRow>, Vec<ProvenanceRow>), AggregationRefusal> {
    let ranking_quantity = ranking_quantity(result, request)?;

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
        let mut ranked: Vec<(String, f64)> = Vec::with_capacity(trial_ids.len());
        for trial_id in &trial_ids {
            let value = result
                .results
                .iter()
                .find(|row| row.trial_id == *trial_id)
                .and_then(|row| row.values.get(&ranking_quantity).copied().flatten())
                .ok_or_else(|| AggregationRefusal::RankingValueAbsent {
                    rule: request.rule.as_registry_str().to_string(),
                    quantity: ranking_quantity.clone(),
                    trial_id: trial_id.clone(),
                    group: group_key.clone(),
                })?;
            ranked.push((trial_id.clone(), value));
        }
        ranked.sort_by(|left, right| {
            right
                .1
                .total_cmp(&left.1)
                .then_with(|| left.0.cmp(&right.0))
        });

        let taken = request.rule.trials_taken();
        if taken < ranked.len() && ranked[taken - 1].1 == ranked[taken].1 {
            let boundary = ranked[taken].1;
            let tied = ranked
                .iter()
                .filter(|(_, value)| *value == boundary)
                .count();
            return Err(AggregationRefusal::RankingTiedAtBoundary {
                rule: request.rule.as_registry_str().to_string(),
                quantity: ranking_quantity.clone(),
                group: group_key,
                value: boundary,
                tied,
            });
        }
        let selected: Vec<&str> = ranked
            .iter()
            .take(taken)
            .map(|(trial_id, _)| trial_id.as_str())
            .collect();

        for quantity in &request.quantities {
            let values: Option<Vec<f64>> = selected
                .iter()
                .map(|trial_id| {
                    result
                        .results
                        .iter()
                        .find(|row| row.trial_id == *trial_id)
                        .and_then(|row| row.values.get(quantity).copied().flatten())
                })
                .collect();
            let values = values.unwrap_or_default();

            let (row, chain) = row_for(request, &group_key, quantity, &values, selected.len());
            rows.push(row);
            chains.extend(chain);
        }
    }
    Ok((rows, chains))
}

/// The constructs this run's results carry in exactly one column, which is the vocabulary
/// `ranked_by` takes on this folder.
///
/// One column, because that is the condition `ranking_quantity` accepts: a construct two
/// columns carry does not identify a value, and no flag on this surface narrows it, so
/// offering one would send a reader from a refusal to a refusal with nothing further to try.
fn constructs_carried(result: &BatchResult) -> Vec<String> {
    let mut columns_per_construct: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for row in result.provenance.iter().filter(|row| row.depth == 0) {
        if !result.quantities.contains(&row.quantity) {
            continue;
        }
        for binding in BINDINGS.iter() {
            if binding.id == row.method_id || records_under(binding.id) == row.method_id {
                columns_per_construct
                    .entry(binding.construct.to_string())
                    .or_default()
                    .insert(row.quantity.clone());
            }
        }
    }
    columns_per_construct
        .into_iter()
        .filter(|(_, columns)| columns.len() == 1)
        .map(|(construct, _)| construct)
        .collect()
}

/// The one result column whose root fills the construct named by `ranked_by`.
///
/// The root is taken from the chain the number carries, not inferred from the column name and
/// not accepted because the rule appears somewhere below it. Two columns rooted in one
/// construct are two possible values, so the required construct id has not chosen between
/// them and the reduction refuses.
fn ranking_quantity(
    result: &BatchResult,
    request: &AggregationRequest,
) -> Result<String, AggregationRefusal> {
    let mut quantities = BTreeSet::new();
    for quantity in &result.quantities {
        let fills_named_construct = result.provenance.iter().any(|row| {
            row.quantity == *quantity
                && row.depth == 0
                && BINDINGS.iter().any(|binding| {
                    binding.construct == request.ranked_by
                        && (binding.id == row.method_id
                            || records_under(binding.id) == row.method_id)
                })
        });
        if fills_named_construct {
            quantities.insert(quantity.clone());
        }
    }

    match quantities.len() {
        0 => Err(AggregationRefusal::RankingConstructAbsent {
            rule: request.rule.as_registry_str().to_string(),
            construct: request.ranked_by.clone(),
            carried: constructs_carried(result),
        }),
        1 => Ok(quantities.into_iter().next().expect("one quantity")),
        _ => Err(AggregationRefusal::RankingConstructAmbiguous {
            rule: request.rule.as_registry_str().to_string(),
            construct: request.ranked_by.clone(),
            quantities: quantities.into_iter().collect(),
        }),
    }
}

/// One reduced value, and the chain that reaches the rule and the count behind it.
fn row_for(
    request: &AggregationRequest,
    group_key: &str,
    quantity: &str,
    reduced: &[f64],
    n: usize,
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
            value: n.to_string(),
            source: source.to_string(),
        },
        ProvenanceRow {
            provenance_id: String::new(),
            quantity: quantity.to_string(),
            depth: 0,
            method_id: method_id.clone(),
            parameter: "ranked_by".to_string(),
            value: request.ranked_by.clone(),
            source: match (request.group_kind, request.rule) {
                (GroupKind::Run, _) => "assumed".to_string(),
                (_, AggregationRule::BestOfNByPeakForce) => "cited".to_string(),
                _ => "stated".to_string(),
            },
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
            n,
            method_id,
            provenance_id: identifier,
        },
        chain,
    )
}

fn dispersion_label(dispersion: DispersionEstimator) -> &'static str {
    dispersion.as_published_str()
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
