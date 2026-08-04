//! Which trials a rule left out of a population, and what the number would have been over
//! the whole set.
//!
//! A gate reports by default and the trial stays in the denominator, so nobody discovers
//! after the fact that their mean was taken over 217 trials and not 244. Applying a gate is a
//! request field recorded in provenance like any other choice, never a constant in a build.

use std::collections::{BTreeMap, BTreeSet};

use plateforce_analysis::AnalysisResponse;
use serde::{Deserialize, Serialize};

/// A whole trial removed from a population by a validity gate.
///
/// Distinct from `plateforce_core::Exclusions`, which is samples dropped inside one trace.
/// One name for both would conflate a trace with a set, which is the confusion the two names
/// exist to prevent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PopulationExclusion {
    pub trial_id: String,
    /// The registry id of the gate, so a count is always attributable to a rule.
    pub method_id: String,
    pub parameter: Option<String>,
    pub value: Option<f64>,
    pub criterion: String,
    /// False when the gate reported and the trial stayed in the denominator.
    pub applied: bool,
}

impl PopulationExclusion {
    /// The flat table, so the relation a reader opens and the record a program reads are one
    /// type rather than two that can come to disagree.
    pub fn header() -> Vec<String> {
        [
            "trial_id",
            "ordinal",
            "method_id",
            "outcome",
            "parameter",
            "value",
            "criterion",
        ]
        .iter()
        .map(|name| name.to_string())
        .collect()
    }

    /// A trial removed from the population keeps its row and all its numbers in `results`, so
    /// this column is the only place a reader can see that no figure over this run was taken
    /// over it. The word rather than the flag, because a reader meeting `false` in a column
    /// called `applied` has to work out what was applied to what.
    pub fn cells(&self, ordinal: usize) -> Vec<String> {
        vec![
            self.trial_id.clone(),
            ordinal.to_string(),
            self.method_id.clone(),
            if self.applied { "removed" } else { "reported" }.to_string(),
            self.parameter.clone().unwrap_or_default(),
            self.value
                .map(crate::relations::format_value)
                .unwrap_or_default(),
            self.criterion.clone(),
        ]
    }
}

/// A rule that decides whether a trial belongs in a population.
///
/// A registry holding no gate is the correct state of a run that bound none.
pub trait ValidityGate {
    /// The registry id this gate is bound to.
    fn method_id(&self) -> &str;

    /// `Some` when the trial matches the gate's criterion. The gate never decides whether the
    /// trial is dropped: that is the request's choice and it is recorded.
    fn examine(&self, trial_id: &str, response: &AnalysisResponse) -> Option<GateFinding>;
}

/// What a gate found, without saying what to do about it.
#[derive(Debug, Clone, PartialEq)]
pub struct GateFinding {
    pub parameter: Option<String>,
    pub value: Option<f64>,
    pub criterion: String,
}

/// Every gate a run bound, with whether each one removes a trial or only names it.
///
/// # Registering a gate
///
/// ```
/// use plateforce_batch::exclusions::{GateFinding, GateRegistry, ValidityGate};
/// use plateforce_analysis::AnalysisResponse;
///
/// struct EveryTrial;
/// impl ValidityGate for EveryTrial {
///     fn method_id(&self) -> &str {
///         "trial.gate.between_trial_agreement.kraska2009"
///     }
///     fn examine(&self, _trial_id: &str, _response: &AnalysisResponse) -> Option<GateFinding> {
///         Some(GateFinding {
///             parameter: Some("permitted_deviation_percent".to_string()),
///             value: Some(10.0),
///             criterion: "the trial sits outside the permitted deviation".to_string(),
///         })
///     }
/// }
///
/// let mut gates = GateRegistry::default();
/// gates.register(Box::new(EveryTrial));
/// assert_eq!(gates.len(), 1);
/// assert_eq!(gates.applied_count(), 0, "a gate reports until a request applies it");
/// ```
#[derive(Default)]
pub struct GateRegistry {
    gates: Vec<Box<dyn ValidityGate>>,
    /// Ids the request asked to apply. A gate absent from this set reports and removes
    /// nothing, which is the only place that choice is expressed.
    applied: BTreeSet<String>,
}

impl GateRegistry {
    pub fn register(&mut self, gate: Box<dyn ValidityGate>) {
        self.gates.push(gate);
    }

    /// Ask for a gate's finding to remove the trial rather than only name it.
    ///
    /// One entry in the registry would reject all 244 trials of this project's own corpus if
    /// it ran unscoped, so a build that applied gates by default could return an empty set
    /// and report success.
    pub fn apply(&mut self, method_id: &str) {
        self.applied.insert(method_id.to_string());
    }

    pub fn len(&self) -> usize {
        self.gates.len()
    }

    pub fn is_empty(&self) -> bool {
        self.gates.is_empty()
    }

    pub fn applied_count(&self) -> usize {
        self.applied.len()
    }

    pub fn reporting_count(&self) -> usize {
        self.gates.len()
    }

    /// Every gate's verdict on one trial.
    pub fn examine(&self, trial_id: &str, response: &AnalysisResponse) -> Vec<PopulationExclusion> {
        self.gates
            .iter()
            .filter_map(|gate| {
                gate.examine(trial_id, response)
                    .map(|finding| PopulationExclusion {
                        trial_id: trial_id.to_string(),
                        method_id: gate.method_id().to_string(),
                        parameter: finding.parameter,
                        value: finding.value,
                        criterion: finding.criterion,
                        applied: self.applied.contains(gate.method_id()),
                    })
            })
            .collect()
    }

    /// Per gate, how many of the denominator it would remove. The shape the committed
    /// baseline is compared against.
    pub fn tally(exclusions: &[PopulationExclusion], denominator: usize) -> Vec<GateTally> {
        let mut counted: BTreeMap<String, (usize, usize)> = BTreeMap::new();
        for exclusion in exclusions {
            let entry = counted.entry(exclusion.method_id.clone()).or_insert((0, 0));
            entry.0 += 1;
            if exclusion.applied {
                entry.1 += 1;
            }
        }
        counted
            .into_iter()
            .map(|(method_id, (would_exclude, applied))| GateTally {
                method_id,
                would_exclude,
                applied,
                denominator,
            })
            .collect()
    }
}

/// One gate's effect on one population, with the denominator it was taken over.
#[derive(Debug, Clone, PartialEq)]
pub struct GateTally {
    pub method_id: String,
    pub would_exclude: usize,
    pub applied: usize,
    pub denominator: usize,
}

impl GateTally {
    pub fn line(&self) -> String {
        format!(
            "{} would exclude {} of {}, applied {} of {}",
            self.method_id, self.would_exclude, self.denominator, self.applied, self.denominator
        )
    }
}
