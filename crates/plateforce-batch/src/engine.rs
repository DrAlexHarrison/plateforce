//! The loop.
//!
//! One analysis over a set of trials, where a file that cannot be read costs its own row and
//! nothing else. Four failure points each produce a named row rather than ending the run,
//! and a trial that declined one landmark while computing the rest carries both.

use std::collections::{BTreeMap, BTreeSet};

use plateforce_analysis::document::refusal_from_rule;
use plateforce_analysis::quality::{QualitySignal, QualityStatus};
use plateforce_analysis::{
    AnalysisRequest, AnalysisResponse, BoundMethod, DeclinedRule, Metric, ONSET_CONSTRUCT,
    ONSET_OPERATOR_IDS, TAKEOFF_CONSTRUCT, WEIGHING_CONSTRUCT,
};
use plateforce_core::{Capture, Refusal, RefusalCode};
use plateforce_registry::Registry;
use serde::Serialize;

use crate::decisions::{unresolved, UnresolvedDecision};
use crate::exclusions::{GateRegistry, PopulationExclusion, ValidityGate};
use crate::fingerprint::{provenance_id, request_digest, run_fingerprint};
use crate::identity::{TrialSet, UnidentifiedFile};
use crate::relations::{
    AggregateRow, ProvenanceRow, RefusalRow, ResultRow, RunRow, SignalRow, WarningRow,
};

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
    /// What the caller knows about the capture, and which saved plate they were told it by.
    /// A trace of forces carries none of it, so it is stated per run rather than read per
    /// file, and a run that states nothing reports every trial as incomplete rather than as
    /// matching.
    pub capture: Option<Capture>,
}

impl BatchRequest {
    pub fn new(analysis: AnalysisRequest) -> Self {
        Self {
            analysis,
            registry_version: None,
            resolved_decisions: BTreeSet::new(),
            gates: GateRegistry::default(),
            capture: None,
        }
    }

    /// State what the capture was, so results from it can be told apart from results whose
    /// capture nobody recorded.
    pub fn describing(mut self, capture: impl Into<Capture>) -> Self {
        self.capture = Some(capture.into());
        self
    }

    /// Cite a registry revision, which is the caller's word and never the registry's own.
    ///
    /// Spelled as `RegistryStamp::pinned_to` spells it, because a caller who has pinned an
    /// `analyse` run and pins a folder run is doing one thing and should write one word.
    pub fn pinned_to(mut self, registry_version: Option<String>) -> Self {
        self.registry_version = registry_version;
        self
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
    /// Files the run met carrying none of the declared suffixes, so the declaration's own
    /// narrowing is a number the reader can see rather than one they would have to take on
    /// trust from the size of the folder.
    pub files_without_declared_suffix: usize,
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
    /// Every file the run met, whatever its name.
    pub fn files_present(&self) -> usize {
        self.files_found + self.files_without_declared_suffix
    }

    /// Printed rather than inferred from a green result, because a run that quietly covered
    /// six trials instead of 244 is the failure this project documents.
    pub fn line(&self) -> String {
        format!(
            "{}, {} of {} named, results {} of {} trials, computed {} of {}, refused {} of {}, excluded {} of {}",
            files_line(self.files_found, self.files_without_declared_suffix),
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

/// What the run was pointed at, and how much of it the declared suffixes kept.
///
/// One sentence for every surface that reports a batch, so a folder cannot be described one
/// way under `analyse` and another under `compare`.
pub(crate) fn files_line(files_found: usize, files_without_declared_suffix: usize) -> String {
    format!(
        "files {}, {files_found} carrying a declared trial suffix and {files_without_declared_suffix} not",
        files_found + files_without_declared_suffix,
    )
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
    /// What the analysis already knew about the numbers it reported, per trial.
    pub signals: Vec<SignalRow>,
    /// Written only when the request bound an aggregation rule.
    pub aggregates: Vec<AggregateRow>,
    /// What each bound gate found, whether or not the request asked it to remove anything.
    pub exclusions: Vec<PopulationExclusion>,
    pub coverage: Coverage,
}

impl BatchResult {
    /// The trials every figure taken over this run is taken over: the ones that produced
    /// numbers, less the ones a gate the request applied removed.
    ///
    /// One home, because two call sites that each decided for themselves what the population
    /// was is how a mean and a reliability figure came to be reported beside a denominator
    /// neither of them was taken over. Ordered as the results table is, so a figure summed
    /// over it adds the same values in the same order as before this existed.
    pub fn population(&self) -> Vec<String> {
        let removed: BTreeSet<&str> = self
            .exclusions
            .iter()
            .filter(|exclusion| exclusion.applied)
            .map(|exclusion| exclusion.trial_id.as_str())
            .collect();
        self.results
            .iter()
            .filter(|row| row.refusal_code.is_empty() && !removed.contains(row.trial_id.as_str()))
            .map(|row| row.trial_id.clone())
            .collect()
    }
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
    // A rule the request cannot bind would be refused identically on every file, so a folder
    // of two hundred is refused once before the first is read, and the caller is told which
    // rules are filed under the construct rather than being told two hundred times.
    for (construct, choice) in &request.analysis.derived {
        if let Err(refusal) = crate::derive::accepts(construct, &choice.method_id) {
            return Err(RunRefusal {
                code: refusal.code,
                message: refusal.message().to_string(),
                unresolved: Vec::new(),
            });
        }
    }

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
    let mut signals: Vec<SignalRow> = Vec::new();
    let mut exclusions: Vec<PopulationExclusion> = Vec::new();
    let mut computed = 0usize;
    let mut refused = 0usize;
    let mut sentinel_rows_total = 0usize;

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

        let (trial, sentinel_rows) = match entry.source.read(&set.format) {
            Ok((trial, _report, dropped)) => (trial, dropped),
            Err(error) => {
                let code = RefusalCode::from(&error).wire_name();
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
            // The code is the one the engine decided it was declining under. This surface
            // used to write `method_not_implemented` on every one of these and leave every
            // other column empty, which named a fault the request had not committed.
            Err(declined) => {
                let code = declined.code.wire_name();
                refusals.push(refusal_row(trial_id, ordinal(&refusals), &declined));
                results.push(refused_row(trial_id, &source_path, code));
                refused += 1;
                continue;
            }
        };

        // A landmark that declined while other numbers computed is the partial state, so the
        // trial carries values and one refusal row per decline at once.
        for declined in &response.refusals {
            refusals.push(rule_refusal_row(trial_id, ordinal(&refusals), declined));
        }
        for (index, sentence) in response.warnings.iter().enumerate() {
            warnings.push(WarningRow {
                trial_id: trial_id.clone(),
                ordinal: index,
                message: sentence.clone(),
            });
        }
        // A number a rule produced from the boundary of its own search is still a number, and
        // it lands in the table beside numbers a rule found in the trace. The analysis has
        // already worked that out by here, so the run carries it rather than recomputing it.
        for (index, signal) in response.signals.iter().enumerate() {
            signals.push(signal_row(trial_id, index, signal));
        }
        // What the reader treated as missing travels with the trial it was taken from, not
        // only as a run total, because a run of 244 that dropped 30 rows in one trace and a
        // run that dropped one row in each are different data and sum the same.
        sentinel_rows_total += sentinel_rows;
        if sentinel_rows > 0 {
            warnings.push(WarningRow {
                trial_id: trial_id.clone(),
                ordinal: response.warnings.len(),
                message: format!(
                    "{sentinel_rows} of {} samples matched the declared missing value and were not read as force",
                    sentinel_rows + trial.len()
                ),
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
            // The first metric under a key, which is the one every other surface reads. Taking
            // the last would write a different number into the batch than the terminal and the
            // quality signals show for the same trial.
            values.entry(metric.key.to_string()).or_insert(metric.value);
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

    // A rule the caller bound that declined on every trial produced no metric, so the column
    // it was asked for would be absent from the table rather than blank in it. Appended after
    // the walk so the columns a run did produce keep the order the analysis reported them in.
    for (key, unit) in crate::derive::declared_quantities(&request.analysis.derived) {
        if !quantities.iter().any(|named| named == key) {
            quantities.push(key.to_string());
            units.insert(key.to_string(), unit.to_string());
        }
    }

    let excluded = exclusions
        .iter()
        .filter(|exclusion| exclusion.applied)
        .map(|exclusion| exclusion.trial_id.clone())
        .collect::<BTreeSet<String>>()
        .len();

    let coverage = Coverage {
        files_found: set.files_found,
        files_without_declared_suffix: set.files_without_declared_suffix,
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

    // The block describes the capture, so it is complete or it is not, once for the run.
    let acquisition = request
        .capture
        .as_ref()
        .map(|capture| capture.acquisition.clone())
        .unwrap_or_default();
    let acquisition_is_complete = acquisition.is_complete();

    let mut run = RunRow {
        plateforce_version: env!("CARGO_PKG_VERSION").to_string(),
        registry_version: request.registry_version.clone(),
        // Read off the registry that was loaded rather than taken from the caller, because
        // this is the registry's claim about itself and no caller can make it.
        registry_declared_version: registry.declared_version.clone(),
        registry_digest: registry.content_digest.clone(),
        request_digest: request_digest(&request.analysis, request.registry_version.as_deref()),
        files_found: coverage.files_found,
        files_without_declared_suffix: coverage.files_without_declared_suffix,
        files_unidentified: coverage.files_unidentified,
        trial_count: coverage.trial_count,
        computed_count: coverage.computed,
        refusal_count: coverage.refused,
        acquisition_complete_count: if acquisition_is_complete {
            coverage.computed
        } else {
            0
        },
        // The block itself, so the row carries what it fingerprinted rather than a count of
        // how many trials it applied to. A run that stated nothing carries the empty block,
        // which is what `Acquisition::missing` names every member of.
        acquisition,
        acquisition_complete: acquisition_is_complete,
        plate_profile: request
            .capture
            .as_ref()
            .and_then(|capture| capture.plate_profile.clone()),
        trials_excluded: coverage.excluded,
        gates_reporting: request.gates.reporting_count(),
        gates_applied: request.gates.applied_count(),
        distinct_provenance_count: provenance_ids.len(),
        trial_identity: set.identity.describe(),
        delimiter: set.format.delimiter.to_string(),
        force_column_index: set.format.force_column_index,
        sample_rate_hz: set.format.sample_rate_hz,
        sentinel: set
            .format
            .sentinel
            .map(crate::relations::format_value)
            .unwrap_or_default(),
        sentinel_rows_dropped: sentinel_rows_total,
        run_fingerprint: None,
    };
    // `published` withholds the digest when the acquisition block was not filled, so a run
    // that cannot be declared to match another carries nothing that could be compared.
    run.run_fingerprint = run_fingerprint(&run, &provenance_ids)
        .published()
        .map(str::to_string);

    Ok(BatchResult {
        run,
        quantities,
        units,
        results,
        provenance: flattened,
        refusals,
        warnings,
        signals,
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

pub(crate) fn unidentified_row(file: &UnidentifiedFile, ordinal: usize) -> RefusalRow {
    RefusalRow {
        trial_id: String::new(),
        ordinal,
        code: RefusalCode::TrialIdentityUnparsed.wire_name().to_string(),
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

/// The typed fields of a rule's refusal, read off the record rather than parsed back out of
/// its sentence.
///
/// The construct and the id come with the decline rather than being recovered here by
/// matching the start of a method id against a table of prefixes. This surface and the
/// document surface each kept a copy of that table and the two copies had different last
/// arms, so an unrecognised name resolved to `bwepoch.` here and to `takeoff.` there.
fn rule_refusal_row(trial_id: &str, ordinal: usize, declined: &DeclinedRule) -> RefusalRow {
    refusal_row(trial_id, ordinal, &refusal_from_rule(declined))
}

/// One writer for every refusal this surface records, so a decline that arrives on a
/// response and one that ends the analysis outright cannot be written into two shapes.
pub(crate) fn refusal_row(trial_id: &str, ordinal: usize, refused: &Refusal) -> RefusalRow {
    RefusalRow {
        trial_id: trial_id.to_string(),
        ordinal,
        code: refused.code.wire_name().to_string(),
        method_id: refused.method_id.clone(),
        slot: refused.slot.clone().unwrap_or_default(),
        parameter: refused.parameter.clone().unwrap_or_default(),
        // A refusal on a name and a refusal on a number both answer "which value", so one
        // column carries whichever of the two this rule declined on.
        value: refused
            .value
            .map(crate::relations::format_value)
            .or_else(|| refused.named_value.clone())
            .unwrap_or_default(),
        detail: refused
            .detail
            .iter()
            .map(|(name, value)| format!("{name}={value}"))
            .collect::<Vec<_>>()
            .join(","),
        available: refused.available.join(","),
        // The sentence the record generates, which is the one carrying the id the boundary
        // stamped on. The rule's own sentence predates that stamp.
        message: refused.message().to_string(),
    }
}

/// One signal as a row, with every field the analysis attached to it.
///
/// Nothing is recomputed and nothing is rephrased: `remedy` is the sentence the analysis
/// composed, and the status is the word the record already carries.
fn signal_row(trial_id: &str, ordinal: usize, signal: &QualitySignal) -> SignalRow {
    SignalRow {
        trial_id: trial_id.to_string(),
        ordinal,
        status: status_name(signal.status),
        label: signal.label.clone(),
        value: signal.value,
        unit: signal.unit.to_string(),
        threshold: signal.threshold,
        qualifies: signal.qualifies.join(","),
        remedy_construct: signal.remedy_construct.to_string(),
        remedy: signal.remedy.clone(),
    }
}

/// The word a status travels under, asked of the status itself rather than spelled again here.
///
/// `QualityStatus::wire_name` is matched exhaustively beside the enum, so this table, the JSON
/// envelope and every other surface cannot disagree about what a status is called, and a status
/// added to the vocabulary is ruled on where it is declared rather than reaching this column
/// blank or under its Rust variant name.
fn status_name(status: QualityStatus) -> String {
    status.wire_name().to_string()
}

/// The chain behind every number the analysis produced.
///
/// Depth 0 is the arithmetic that made the quantity where the response names one, the
/// landmark rules sit one below it, and an operator composed onto a landmark rule sits one
/// below that.
///
/// The arithmetic's own values are read off the same bound record the landmarks' are. It is
/// named in `computed_by` and not in `contributing_method_ids`, so a chain written from the
/// contributing list alone carried its id and none of what it read: the gravity behind the
/// flight-time height and the four integration choices behind every impulse figure reached
/// the terminal's record and no folder run's.
fn provenance_rows(response: &AnalysisResponse) -> Vec<ProvenanceRow> {
    let mut rows = Vec::new();
    for metric in &response.metrics {
        if metric.value.is_none() {
            continue;
        }
        let base_depth = usize::from(metric.computed_by.is_some());
        if let Some(arithmetic) = &metric.computed_by {
            match response
                .bound_methods
                .iter()
                .find(|bound| bound.method_id == *arithmetic)
            {
                Some(bound) => rows.extend(rows_for_bound_method(metric, bound, 0)),
                // A rule the response named and left no bound record for still opens the
                // chain, because dropping it would put the landmarks under nothing.
                None => rows.push(ProvenanceRow {
                    provenance_id: String::new(),
                    quantity: metric.key.to_string(),
                    depth: 0,
                    method_id: arithmetic.to_string(),
                    parameter: String::new(),
                    value: String::new(),
                    source: String::new(),
                }),
            }
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
