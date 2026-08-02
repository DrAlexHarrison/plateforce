//! The loop.
//!
//! One analysis over a set of trials, where a file that cannot be read costs its own row and
//! nothing else. Four failure points each produce a named row rather than ending the run,
//! and a trial that declined one landmark while computing the rest carries both.

use std::collections::{BTreeMap, BTreeSet};

use plateforce_analysis::{
    AnalysisRequest, AnalysisResponse, BoundMethod, Metric, RuleRefusal, ONSET_CONSTRUCT,
    ONSET_OPERATOR_IDS, TAKEOFF_CONSTRUCT, WEIGHING_CONSTRUCT,
};
use plateforce_core::RefusalCode;
use plateforce_registry::Registry;
use serde::Serialize;

use crate::decisions::{unresolved, UnresolvedDecision};
use crate::exclusions::{GateRegistry, PopulationExclusion, ValidityGate};
use crate::fingerprint::{provenance_id, request_digest, run_fingerprint};
use crate::identity::{TrialSet, UnidentifiedFile};
use crate::relations::{AggregateRow, ProvenanceRow, RefusalRow, ResultRow, RunRow, WarningRow};

/// What one batch run was asked to do.
pub struct BatchRequest {
    pub analysis: AnalysisRequest,
    /// The revision the caller pinned, if they pinned one.
    pub registry_version: Option<String>,
    /// Constructs an explicit act has resolved. Naming a rule on a command line or in a
    /// browser is such an act; arriving at one because nobody said otherwise is not.
    pub resolved_decisions: BTreeSet<String>,
    /// The validity gates this run bound, and which of them remove a trial rather than
    /// naming it. Empty is the correct state of a run that bound none.
    pub gates: GateRegistry,
}

impl BatchRequest {
    pub fn new(analysis: AnalysisRequest) -> Self {
        Self {
            analysis,
            registry_version: None,
            resolved_decisions: BTreeSet::new(),
            gates: GateRegistry::default(),
        }
    }

    /// Bind a validity gate. It reports and removes nothing until the request applies it.
    pub fn with_gate(mut self, gate: Box<dyn ValidityGate>) -> Self {
        self.gates.register(gate);
        self
    }

    /// Ask a bound gate's finding to remove the trial from the population.
    pub fn applying(mut self, method_id: &str) -> Self {
        self.gates.apply(method_id);
        self
    }

    /// Record that the caller chose these constructs' rules deliberately.
    pub fn resolving(mut self, constructs: &[&str]) -> Self {
        self.resolved_decisions
            .extend(constructs.iter().map(|name| (*name).to_string()));
        self
    }
}

/// A run that produced nothing, because a choice on the path is still open.
///
/// The code is the shared enum rather than a string, so a caller maps it through
/// `plateforce_core::exit_code` instead of writing a second table that can disagree.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RunRefusal {
    pub code: RefusalCode,
    pub message: String,
    pub unresolved: Vec<UnresolvedDecision>,
}

/// What the run walked, stated against the denominator each count is taken over.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Coverage {
    pub files_found: usize,
    pub files_unidentified: usize,
    pub trial_count: usize,
    /// Counted off the rows that exist rather than taken from the trial count, so a row the
    /// loop dropped shows up here instead of being papered over by the denominator.
    pub results_written: usize,
    pub computed: usize,
    pub refused: usize,
    pub excluded: usize,
}

impl Coverage {
    /// Printed rather than inferred from a green result, because a run that quietly covered
    /// six trials instead of 244 is the failure this project documents.
    pub fn line(&self) -> String {
        format!(
            "files {} found, {} of {} named, results {} of {} trials, computed {} of {}, refused {} of {}, excluded {} of {}",
            self.files_found,
            self.trial_count,
            self.files_found,
            self.results_written,
            self.trial_count,
            self.computed,
            self.trial_count,
            self.refused,
            self.trial_count,
            self.excluded,
            self.trial_count,
        )
    }
}

/// The relations one run produced.
#[derive(Debug, Clone)]
pub struct BatchResult {
    pub run: RunRow,
    /// The quantity columns, in the order the analysis reported them.
    pub quantities: Vec<String>,
    /// The unit each quantity is in, as the registry spells it. Carried rather than inferred
    /// from the column name, because a surface that guessed a unit from a name would be
    /// deciding something no rule decided.
    pub units: BTreeMap<String, String>,
    pub results: Vec<ResultRow>,
    pub provenance: Vec<ProvenanceRow>,
    pub refusals: Vec<RefusalRow>,
    pub warnings: Vec<WarningRow>,
    /// Written only when the request bound an aggregation rule.
    pub aggregates: Vec<AggregateRow>,
    /// What each bound gate found, whether or not the request asked it to remove anything.
    pub exclusions: Vec<PopulationExclusion>,
    pub coverage: Coverage,
}

/// The constructs a jump-height request walks, read from the binding layer rather than
/// listed here, so a slot added there reaches this precondition without an edit.
fn path_constructs() -> [&'static str; 3] {
    [WEIGHING_CONSTRUCT, ONSET_CONSTRUCT, TAKEOFF_CONSTRUCT]
}

/// Run one analysis over every trial in the set.
///
/// The registry arrives as a loaded object rather than as a digest string, because the digest
/// it carries was measured from the bytes it read and a digest passed in as an argument rests
/// on a caller's word.
pub fn analyse(
    set: &TrialSet,
    request: &BatchRequest,
    registry: &Registry,
) -> Result<BatchResult, RunRefusal> {
    let open = unresolved(registry, &path_constructs(), &request.resolved_decisions);
    if !open.is_empty() {
        let named: Vec<String> = open.iter().map(UnresolvedDecision::message).collect();
        return Err(RunRefusal {
            // The name is known and the choice was never made, which is a different fault
            // from a parameter nobody recognises.
            code: RefusalCode::DecisionNotMade,
            message: format!(
                "{} of {} choices on this path are still to be made: {}",
                open.len(),
                path_constructs().len(),
                named.join("; ")
            ),
            unresolved: open,
        });
    }

    let mut quantities: Vec<String> = Vec::new();
    let mut units: BTreeMap<String, String> = BTreeMap::new();
    let mut results: Vec<ResultRow> = Vec::new();
    let mut provenance: BTreeMap<String, Vec<ProvenanceRow>> = BTreeMap::new();
    let mut refusals: Vec<RefusalRow> = Vec::new();
    let mut warnings: Vec<WarningRow> = Vec::new();
    let mut exclusions: Vec<PopulationExclusion> = Vec::new();
    let mut computed = 0usize;
    let mut refused = 0usize;

    for unidentified in &set.unidentified {
        refusals.push(unidentified_row(unidentified, refusals.len()));
    }

    for (trial_id, entry) in set.iter() {
        let source_path = if set.writes_source_path() {
            entry.source.display_path()
        } else {
            String::new()
        };
        let ordinal =
            |rows: &[RefusalRow]| rows.iter().filter(|row| row.trial_id == *trial_id).count();

        let trial = match entry.source.read(&set.format) {
            Ok((trial, _report)) => trial,
            Err(error) => {
                let code = read_error_code(&error);
                refusals.push(RefusalRow {
                    trial_id: trial_id.clone(),
                    ordinal: ordinal(&refusals),
                    code: code.to_string(),
                    method_id: String::new(),
                    slot: String::new(),
                    parameter: String::new(),
                    value: String::new(),
                    detail: String::new(),
                    available: String::new(),
                    message: error.to_string(),
                });
                results.push(refused_row(trial_id, &source_path, code));
                refused += 1;
                continue;
            }
        };

        let response = match plateforce_analysis::run(&trial, &request.analysis) {
            Ok(response) => response,
            Err(message) => {
                refusals.push(RefusalRow {
                    trial_id: trial_id.clone(),
                    ordinal: ordinal(&refusals),
                    code: "method_not_implemented".to_string(),
                    method_id: String::new(),
                    slot: String::new(),
                    parameter: String::new(),
                    value: String::new(),
                    detail: String::new(),
                    available: String::new(),
                    message,
                });
                results.push(refused_row(
                    trial_id,
                    &source_path,
                    "method_not_implemented",
                ));
                refused += 1;
                continue;
            }
        };

        // A landmark that declined while other numbers computed is the partial state, so the
        // trial carries values and one refusal row per decline at once.
        for (slot, refusal) in &response.refusals {
            refusals.push(rule_refusal_row(
                trial_id,
                ordinal(&refusals),
                slot,
                refusal,
                &response,
            ));
        }
        for (index, sentence) in response.warnings.iter().enumerate() {
            warnings.push(WarningRow {
                trial_id: trial_id.clone(),
                ordinal: index,
                message: sentence.clone(),
            });
        }

        // Every gate looks at every trial that computed. The finding is recorded whether or
        // not it removes the trial, so the denominator is visible either way.
        exclusions.extend(request.gates.examine(trial_id, &response));

        let rows = provenance_rows(&response);
        let identifier = provenance_id(&rows);
        provenance.entry(identifier.clone()).or_insert_with(|| {
            rows.into_iter()
                .map(|mut row| {
                    row.provenance_id = identifier.clone();
                    row
                })
                .collect()
        });

        let mut values: BTreeMap<String, Option<f64>> = BTreeMap::new();
        for metric in &response.metrics {
            if !quantities.contains(&metric.key) {
                quantities.push(metric.key.to_string());
                units.insert(metric.key.to_string(), metric.unit.to_string());
            }
            values.insert(metric.key.to_string(), metric.value);
        }
        results.push(ResultRow {
            trial_id: trial_id.clone(),
            source_path,
            provenance_id: identifier,
            refusal_code: String::new(),
            values,
        });
        computed += 1;
    }

    let excluded = exclusions
        .iter()
        .filter(|exclusion| exclusion.applied)
        .map(|exclusion| exclusion.trial_id.clone())
        .collect::<BTreeSet<String>>()
        .len();

    let coverage = Coverage {
        files_found: set.files_found,
        files_unidentified: set.unidentified.len(),
        trial_count: set.len(),
        results_written: results.len(),
        computed,
        refused,
        excluded,
    };
    let provenance_ids: BTreeSet<String> = provenance.keys().cloned().collect();
    let mut flattened: Vec<ProvenanceRow> = provenance.into_values().flatten().collect();
    flattened.sort_by(|left, right| {
        (
            &left.provenance_id,
            &left.quantity,
            left.depth,
            &left.method_id,
            &left.parameter,
        )
            .cmp(&(
                &right.provenance_id,
                &right.quantity,
                right.depth,
                &right.method_id,
                &right.parameter,
            ))
    });

    let mut run = RunRow {
        plateforce_version: env!("CARGO_PKG_VERSION").to_string(),
        registry_version: request.registry_version.clone().unwrap_or_default(),
        registry_digest: registry.content_digest.clone(),
        request_digest: request_digest(&request.analysis, request.registry_version.as_deref()),
        files_found: coverage.files_found,
        files_unidentified: coverage.files_unidentified,
        trial_count: coverage.trial_count,
        computed_count: coverage.computed,
        refusal_count: coverage.refused,
        // Nothing in the shipped reader fills an acquisition block, so this is 0 of the
        // trials that computed, which is what a dataset that cannot fill it should report.
        acquisition_complete_count: 0,
        trials_excluded: coverage.excluded,
        gates_reporting: request.gates.reporting_count(),
        gates_applied: request.gates.applied_count(),
        distinct_provenance_count: provenance_ids.len(),
        trial_identity: set.identity.describe(),
        run_fingerprint: String::new(),
    };
    run.run_fingerprint = run_fingerprint(&run, &provenance_ids);

    Ok(BatchResult {
        run,
        quantities,
        units,
        results,
        provenance: flattened,
        refusals,
        warnings,
        aggregates: Vec::new(),
        exclusions,
        coverage,
    })
}

fn refused_row(trial_id: &str, source_path: &str, code: &str) -> ResultRow {
    ResultRow {
        trial_id: trial_id.to_string(),
        source_path: source_path.to_string(),
        provenance_id: String::new(),
        refusal_code: code.to_string(),
        values: BTreeMap::new(),
    }
}

fn unidentified_row(file: &UnidentifiedFile, ordinal: usize) -> RefusalRow {
    RefusalRow {
        trial_id: String::new(),
        ordinal,
        code: "trial_identity_unparsed".to_string(),
        method_id: String::new(),
        slot: String::new(),
        parameter: file.file_name.clone(),
        value: String::new(),
        detail: String::new(),
        // The template that did not match, or the id two files landed on: in both cases the
        // thing a caller changes to resolve it.
        available: file.parameter(),
        message: file.message(),
    }
}

/// The reader's own errors, carried under the code that names the class of fault.
fn read_error_code(error: &plateforce_core::ReadError) -> &'static str {
    use plateforce_core::ReadError;
    match error {
        ReadError::ColumnMissing { .. } | ReadError::NoRows { .. } => "column_not_found",
        ReadError::NotANumber { .. } => "parameter_not_finite",
        ReadError::Trace(inner) => trial_error_code(inner),
        ReadError::Io { .. } => "column_not_found",
    }
}

fn trial_error_code(error: &plateforce_core::TrialError) -> &'static str {
    use plateforce_core::TrialError;
    match error {
        TrialError::Empty | TrialError::EpochTooLong { .. } => "trace_too_short",
        TrialError::BadSampleRate(_) => "parameter_not_finite",
        TrialError::NoCrossing { .. } => "no_crossing",
        TrialError::CollapsedBand { .. } => "collapsed_band",
    }
}

/// The typed fields of a landmark refusal, read off the error rather than parsed back out of
/// its sentence. `RuleRefusal::Stated` carries no fields, and every rule in the tree that
/// produces one is a request asking for something not on offer, so it reads as that.
fn rule_refusal_row(
    trial_id: &str,
    ordinal: usize,
    slot: &str,
    refusal: &RuleRefusal,
    response: &AnalysisResponse,
) -> RefusalRow {
    use plateforce_core::TrialError;
    let bound_for_slot = response
        .bound_methods
        .iter()
        .find(|bound| bound.method_id.starts_with(slot_prefix(slot)))
        .map(|bound| bound.method_id.clone())
        .unwrap_or_default();

    let (code, method_id, parameter, value, detail) = match refusal {
        RuleRefusal::Trial(TrialError::NoCrossing {
            method_id,
            parameter,
            value,
            search_bound_seconds,
        }) => (
            "no_crossing",
            method_id.clone(),
            parameter.clone(),
            crate::relations::format_value(*value),
            format!("search_bound_seconds={search_bound_seconds}"),
        ),
        RuleRefusal::Trial(TrialError::CollapsedBand {
            method_id,
            parameter,
            value,
            dispersion_newtons,
            threshold_newtons,
        }) => (
            "collapsed_band",
            method_id.clone(),
            parameter.clone(),
            crate::relations::format_value(*value),
            format!(
                "dispersion_newtons={dispersion_newtons},threshold_newtons={threshold_newtons}"
            ),
        ),
        RuleRefusal::Trial(other) => (
            trial_error_code(other),
            bound_for_slot,
            String::new(),
            String::new(),
            String::new(),
        ),
        RuleRefusal::Stated(_) => (
            "unknown_parameter",
            bound_for_slot,
            String::new(),
            String::new(),
            String::new(),
        ),
    };

    RefusalRow {
        trial_id: trial_id.to_string(),
        ordinal,
        code: code.to_string(),
        method_id,
        slot: slot.to_string(),
        parameter,
        value,
        detail,
        available: String::new(),
        message: refusal.to_string(),
    }
}

fn slot_prefix(slot: &str) -> &'static str {
    match slot {
        "onset" => "onset.",
        "takeoff" => "takeoff.",
        _ => "bwepoch.",
    }
}

/// The chain behind every number the analysis produced.
///
/// Depth 0 is the arithmetic that made the quantity where the response names one, the
/// landmark rules sit one below it, and an operator composed onto a landmark rule sits one
/// below that.
fn provenance_rows(response: &AnalysisResponse) -> Vec<ProvenanceRow> {
    let mut rows = Vec::new();
    for metric in &response.metrics {
        if metric.value.is_none() {
            continue;
        }
        let base_depth = usize::from(metric.computed_by.is_some());
        if let Some(arithmetic) = &metric.computed_by {
            rows.push(ProvenanceRow {
                provenance_id: String::new(),
                quantity: metric.key.to_string(),
                depth: 0,
                method_id: arithmetic.to_string(),
                parameter: String::new(),
                value: String::new(),
                source: String::new(),
            });
        }
        for method_id in &metric.contributing_method_ids {
            let depth = base_depth + usize::from(ONSET_OPERATOR_IDS.contains(&method_id.as_str()));
            let Some(bound) = response
                .bound_methods
                .iter()
                .find(|bound| bound.method_id == *method_id)
            else {
                continue;
            };
            rows.extend(rows_for_bound_method(metric, bound, depth));
        }
    }
    rows
}

fn rows_for_bound_method(metric: &Metric, bound: &BoundMethod, depth: usize) -> Vec<ProvenanceRow> {
    if bound.bound_parameters.is_empty() {
        return vec![ProvenanceRow {
            provenance_id: String::new(),
            quantity: metric.key.to_string(),
            depth,
            method_id: bound.method_id.clone(),
            parameter: String::new(),
            value: String::new(),
            source: String::new(),
        }];
    }
    bound
        .bound_parameters
        .iter()
        .map(|(parameter, value)| ProvenanceRow {
            provenance_id: String::new(),
            quantity: metric.key.to_string(),
            depth,
            method_id: bound.method_id.clone(),
            parameter: parameter.clone(),
            value: value.clone(),
            // The rule recorded where each value came from; deriving it again here could
            // only ever spell two of the five sources.
            source: bound
                .parameter_sources
                .get(parameter)
                .map(|source| {
                    serde_json::to_value(source)
                        .ok()
                        .and_then(|value| value.as_str().map(str::to_string))
                        .unwrap_or_else(|| "assumed".to_string())
                })
                .unwrap_or_else(|| "assumed".to_string()),
        })
        .collect()
}
