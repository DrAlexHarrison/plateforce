//! The loop.
//!
//! One analysis over a set of trials, where a file that cannot be read costs its own row and
//! nothing else. Four failure points each produce a named row rather than ending the run,
//! and a trial that declined one landmark while computing the rest carries both.

use std::collections::{BTreeMap, BTreeSet};

use plateforce_analysis::document::refusal_from_rule;
use plateforce_analysis::quality::{QualitySignal, QualityStatus};
use plateforce_analysis::{
    chain_of, AnalysisRequest, AnalysisResponse, BoundMethod, DeclinedRule, ONSET_CONSTRUCT,
    TAKEOFF_CONSTRUCT, WEIGHING_CONSTRUCT,
};
use plateforce_core::provenance::{ParameterSource, RegistryStamp};
use plateforce_core::{Capture, ProvenanceChain, Refusal, RefusalCode};
use plateforce_registry::Registry;
use serde::Serialize;

use crate::decisions::{unresolved, UnresolvedDecision, UnresolvedValue};
use crate::exclusions::{GateRegistry, PopulationExclusion, ValidityGate};
use crate::fingerprint::{provenance_id, request_digest, run_fingerprint};
use crate::identity::{TrialSet, UnidentifiedFile};
use crate::relations::{
    AggregateRow, DescriptionRow, ProvenanceRow, RefusalRow, ResultRow, RunRow, SignalRow,
    WarningRow,
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
    /// The athlete's mass per subject, where a folder holds more than one athlete. Keyed by
    /// the subject a declared pattern pulled out of each file name.
    ///
    /// Empty is the correct state of a folder holding one athlete, whose mass sits on the
    /// analysis request beside the gravity. The plate and the acquisition block are stated
    /// once for the folder because they describe the recording; a mass describes the person,
    /// and this set already spans subjects everywhere else it is read. `Session::group` takes
    /// every reliability figure over the subject a pattern named, so a squad session was
    /// already a folder this software understood, in every field but this one.
    pub body_mass_kilograms_by_subject: BTreeMap<String, f64>,
}

impl BatchRequest {
    pub fn new(analysis: AnalysisRequest) -> Self {
        Self {
            analysis,
            registry_version: None,
            resolved_decisions: BTreeSet::new(),
            gates: GateRegistry::default(),
            capture: None,
            body_mass_kilograms_by_subject: BTreeMap::new(),
        }
    }

    /// State a mass per subject, for a folder holding more than one athlete.
    ///
    /// A folder that states these leaves the analysis request's own mass unset, so no trial
    /// runs under a mass belonging to somebody else and no record claims one.
    pub fn massing(mut self, by_subject: BTreeMap<String, f64>) -> Self {
        self.analysis.body_mass_kilograms = None;
        self.body_mass_kilograms_by_subject = by_subject;
        self
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
    /// Values a bound rule requires that the literature publishes several ways and nobody
    /// named. A construct with no rule is the list above; a rule whose number is still open
    /// is this one, and both refuse the run.
    #[serde(default)]
    pub unresolved_values: Vec<UnresolvedValue>,
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
    /// six trials instead of 244 succeeds the same way.
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
    /// The account each number in `results` gives of itself, one row per trial per quantity
    /// that produced a value.
    pub descriptions: Vec<DescriptionRow>,
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
    /// One home, so a mean and a reliability figure cannot be reported beside a denominator
    /// neither of them was taken over. Ordered as the results table is, so a figure summed
    /// over it adds the same values in the same order.
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

/// Each construct on the path with the rule it holds and the values the request stated
/// against it, which is what deciding whether a number is still open takes.
///
/// The three landmark steps, which is the set the single trial asks the same question of. A
/// construct computed from the landmarks is a separate question and neither surface asks it,
/// so asking it here would put the two back out of step.
fn values_on_the_path(request: &BatchRequest) -> Vec<(&'static str, &str, &BTreeMap<String, f64>)> {
    let analysis = &request.analysis;
    vec![
        (
            WEIGHING_CONSTRUCT,
            analysis.weighing.method_id.as_str(),
            &analysis.weighing.parameters,
        ),
        (
            ONSET_CONSTRUCT,
            analysis.onset.method_id.as_str(),
            &analysis.onset.parameters,
        ),
        (
            TAKEOFF_CONSTRUCT,
            analysis.takeoff.method_id.as_str(),
            &analysis.takeoff.parameters,
        ),
    ]
}

/// How many values on this path the literature publishes more than one way, which is the
/// denominator the open count is taken over.
fn published_more_than_one_way(registry: &Registry, request: &BatchRequest) -> usize {
    crate::decisions::values_forcing_a_choice(registry, &values_on_the_path(request))
}

/// Every subject the folder holds, which a declared pattern named and a file stem did not.
fn subjects_present(set: &TrialSet) -> BTreeSet<&str> {
    set.iter()
        .filter_map(|(_, entry)| entry.subject.as_ref())
        .map(|key| key.subject.as_str())
        .collect()
}

/// Whether the masses stated per subject and the subjects the folder holds are the same set.
///
/// Both directions refuse, because both are silent otherwise. A mass written against a name
/// the folder does not hold applies to nothing, and a subject the map does not cover runs at
/// no mass at all while the record beside it lists a mass for every other athlete, which reads
/// as coverage. Refused once for the folder rather than once per trial.
fn masses_cover_the_folder(set: &TrialSet, request: &BatchRequest) -> Result<(), RunRefusal> {
    if request.body_mass_kilograms_by_subject.is_empty() {
        return Ok(());
    }
    let present = subjects_present(set);
    let named: BTreeSet<&str> = request
        .body_mass_kilograms_by_subject
        .keys()
        .map(String::as_str)
        .collect();
    let refused = |code: RefusalCode, message: String| RunRefusal {
        code,
        message,
        unresolved: Vec::new(),
        unresolved_values: Vec::new(),
    };

    let unknown: Vec<&str> = named.difference(&present).copied().collect();
    if !unknown.is_empty() {
        return Err(refused(
            RefusalCode::ValueNotAccepted,
            format!(
                "{} of {} masses name an athlete not in this folder: {}\n  {}",
                unknown.len(),
                named.len(),
                unknown.join(", "),
                if present.is_empty() {
                    "it names no athlete, so --pattern gives it one".to_string()
                } else {
                    format!(
                        "it holds {}",
                        present.iter().copied().collect::<Vec<&str>>().join(", ")
                    )
                }
            ),
        ));
    }
    let uncovered: Vec<&str> = present.difference(&named).copied().collect();
    if !uncovered.is_empty() {
        return Err(refused(
            RefusalCode::RequiredParameterUnstated,
            format!(
                "{} of {} subjects in this folder have no mass: {}",
                uncovered.len(),
                present.len(),
                uncovered.join(", ")
            ),
        ));
    }
    Ok(())
}

/// Each subject's mass as the record carries it.
///
/// The row is built by asking a request bound to that one mass what globals it holds, so the
/// unit, the symbol and the claim about who chose it come from the one place that answers
/// that question rather than from a second set of literals here.
fn mass_rows(request: &BatchRequest) -> BTreeMap<String, crate::relations::BoundGlobalRow> {
    request
        .body_mass_kilograms_by_subject
        .iter()
        .filter_map(|(subject, kilograms)| {
            let mut one = request.analysis.clone();
            one.body_mass_kilograms = Some(*kilograms);
            one.bound_globals()
                .iter()
                .find(|bound| bound.name == plateforce_analysis::BODY_MASS_GLOBAL)
                .map(|bound| (subject.clone(), crate::relations::BoundGlobalRow::of(bound)))
        })
        .collect()
}

/// The request one trial runs under.
///
/// The folder's own, unless the caller stated a mass per subject, in which case this trial's
/// athlete's mass reaches its analysis and nobody else's does. Borrowed where the folder holds
/// one athlete, so the common run clones nothing.
fn analysis_for<'a>(
    entry: &crate::identity::TrialEntry,
    request: &'a BatchRequest,
) -> std::borrow::Cow<'a, AnalysisRequest> {
    if request.body_mass_kilograms_by_subject.is_empty() {
        return std::borrow::Cow::Borrowed(&request.analysis);
    }
    let mut own = request.analysis.clone();
    own.body_mass_kilograms = entry.subject.as_ref().and_then(|key| {
        request
            .body_mass_kilograms_by_subject
            .get(&key.subject)
            .copied()
    });
    std::borrow::Cow::Owned(own)
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
                unresolved_values: Vec::new(),
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
            unresolved_values: Vec::new(),
        });
    }

    // Naming the rule does not always close the choice. A rule requiring a number the
    // literature publishes several ways leaves the number open, and a folder multiplies that
    // by the trial count into a spreadsheet nobody re-reads the provenance of. The terminal
    // has refused this since it shipped and the folder ran it at whichever value the code
    // held, recording that nobody was asked, which is one request answered two ways.
    let values = crate::decisions::unresolved_values(registry, &values_on_the_path(request));
    if !values.is_empty() {
        let named: Vec<String> = values.iter().map(UnresolvedValue::message).collect();
        return Err(RunRefusal {
            code: RefusalCode::DecisionNotMade,
            message: format!(
                "{} of {} values on this path are published more than one way and were not named: {}",
                values.len(),
                published_more_than_one_way(registry, request),
                named.join("; ")
            ),
            unresolved: Vec::new(),
            unresolved_values: values,
        });
    }

    masses_cover_the_folder(set, request)?;

    // What every record this run produces says about the registry behind it, and whether the
    // plate's settings were recorded. Both are facts about the run rather than about a trial,
    // so they are read once here rather than per trial inside the loop.
    let stamp = RegistryStamp {
        version: request.registry_version.clone(),
        // The registry's claim about itself, off the registry that was loaded. A caller
        // cannot make this claim, which is why it is not the pin above.
        declared_version: registry.declared_version.clone(),
        digest: Some(registry.content_digest.clone()),
    };
    // The block describes the capture, so it is complete or it is not, once for the run.
    let acquisition = request
        .capture
        .as_ref()
        .map(|capture| capture.acquisition.clone())
        .unwrap_or_default();
    let acquisition_is_complete = acquisition.is_complete();

    let mut quantities: Vec<String> = Vec::new();
    let mut units: BTreeMap<String, String> = BTreeMap::new();
    let mut results: Vec<ResultRow> = Vec::new();
    let mut provenance: BTreeMap<String, Vec<ProvenanceRow>> = BTreeMap::new();
    let mut descriptions: Vec<DescriptionRow> = Vec::new();
    let mut refusals: Vec<RefusalRow> = Vec::new();
    let mut warnings: Vec<WarningRow> = Vec::new();
    let mut signals: Vec<SignalRow> = Vec::new();
    let mut exclusions: Vec<PopulationExclusion> = Vec::new();
    let mut computed = 0usize;
    let mut refused = 0usize;
    let mut matched_the_convention_total = 0usize;
    let mut carried_no_number_total = 0usize;

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

        let (trial, reported) = match entry.source.read(&set.format) {
            Ok((trial, _report, reported)) => (trial, reported),
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
                results.push(refused_row(
                    trial_id,
                    &subject_of(entry),
                    &source_path,
                    code,
                ));
                refused += 1;
                continue;
            }
        };

        let analysis = analysis_for(entry, request);
        let response = match plateforce_analysis::run(&trial, &analysis) {
            Ok(response) => response,
            // The code is the one the engine decided it was declining under.
            Err(declined) => {
                let code = declined.code.wire_name();
                refusals.push(refusal_row(trial_id, ordinal(&refusals), &declined));
                results.push(refused_row(
                    trial_id,
                    &subject_of(entry),
                    &source_path,
                    code,
                ));
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
        // What the reader could not take as a measurement travels with the trial it was taken
        // from, not only as a run total, because a run of 244 that reported 30 samples in one
        // trace and a run that reported one in each are different data and sum the same.
        //
        // The two reasons are carried apart. One total cannot say whether a run met the
        // convention a caller declared or a gap in the recording, and on a jump trace under
        // the zero convention most of the first is an athlete in the air.
        matched_the_convention_total += reported.matched_the_convention;
        carried_no_number_total += reported.carried_no_number;
        if reported.total() > 0 {
            warnings.push(WarningRow {
                trial_id: trial_id.clone(),
                ordinal: response.warnings.len(),
                message: format!(
                    "of {} samples, {} match the declared missing value and {} carry no number, all kept where they are",
                    trial.len(),
                    reported.matched_the_convention,
                    reported.carried_no_number
                ),
            });
        }

        // Every gate looks at every trial that computed. The finding is recorded whether or
        // not it removes the trial, so the denominator is visible either way.
        exclusions.extend(request.gates.examine(trial_id, &response));

        let rows = provenance_rows(&response, &stamp, acquisition_is_complete);
        let identifier = provenance_id(&rows);
        provenance.entry(identifier.clone()).or_insert_with(|| {
            rows.into_iter()
                .map(|mut row| {
                    row.provenance_id = identifier.clone();
                    row
                })
                .collect()
        });

        // The account each of this trial's numbers gives of itself, from the one site that
        // writes them. A folder run wrote none, so a reader who ran two hundred trials held
        // the rules as rows and no number's own account of itself.
        for (quantity, account) in
            plateforce_analysis::accounts_of(&response, &stamp, acquisition_is_complete)
        {
            descriptions.push(DescriptionRow {
                trial_id: trial_id.clone(),
                quantity,
                provenance_id: identifier.clone(),
                account,
            });
        }

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
            // Read off the entry the walk already resolved rather than parsed again here.
            subject: subject_of(entry),
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

    let mut run = RunRow {
        plateforce_version: env!("CARGO_PKG_VERSION").to_string(),
        // The one stamp every record on this run was written against, so the row and the
        // chains cannot name two registries.
        registry_version: stamp.version.clone(),
        registry_declared_version: stamp.declared_version.clone(),
        // A run always read a registry, so the row states the digest rather than admitting
        // absence. The stamp admits it because a record can be written without one.
        registry_digest: registry.content_digest.clone(),
        request_digest: request_digest(
            &request.analysis,
            request.registry_version.as_deref(),
            &request.body_mass_kilograms_by_subject,
        ),
        // Read off the request the folder ran under, so the row names the same values every
        // trial's analysis was handed rather than a second account of them.
        bound_globals: request
            .analysis
            .bound_globals()
            .iter()
            .map(crate::relations::BoundGlobalRow::of)
            .collect(),
        // Written through the same row type and the same claim about who chose the value, so
        // one athlete's mass and a squad's are one record in two shapes rather than two
        // records. Empty on a folder holding one athlete, whose mass is above.
        body_mass_kilograms_by_subject: mass_rows(request),
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
        samples_matching_the_convention: matched_the_convention_total,
        samples_carrying_no_number: carried_no_number_total,
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
        descriptions,
        refusals,
        warnings,
        signals,
        aggregates: Vec::new(),
        exclusions,
        coverage,
    })
}

/// The athlete a walked trial belongs to, or empty where the run declared no pattern.
///
/// One reading, so a refused row and a computed row cannot come to disagree about whose trial
/// they describe.
fn subject_of(entry: &crate::identity::TrialEntry) -> String {
    entry
        .subject
        .as_ref()
        .map(|key| key.subject.clone())
        .unwrap_or_default()
}

/// A trial that produced no numbers still belongs to the athlete it was recorded on.
///
/// So the subject travels on a refused row too. Without it, grouping by athlete would count
/// only that athlete's trials that computed, and the count would read as their whole session:
/// a silent exclusion of exactly the trials a reader most needs to see.
fn refused_row(trial_id: &str, subject: &str, source_path: &str, code: &str) -> ResultRow {
    ResultRow {
        trial_id: trial_id.to_string(),
        subject: subject.to_string(),
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

/// The chain behind every number the analysis produced, as one row per bound value.
///
/// The tree is `plateforce_analysis::chain_of`'s and not this surface's. It used to be built
/// here from the two flat lists a response carries, and three other surfaces built it from the
/// same two lists in three other shapes, so one number arrived at a folder run and at a
/// notebook resting on different rules.
///
/// `depth` is the step's depth in that tree: the arithmetic that made the quantity roots it
/// where the response names one, the rules its answer rests on sit below, and an operator sits
/// under the landmark rule it composes onto.
///
/// Each row's text is the rule's own, read off the bound record the step names rather than
/// formatted again from the number, because this surface and the engine spell a whole number
/// differently and re-rendering it here would rewrite every value in the relation.
fn provenance_rows(
    response: &AnalysisResponse,
    registry: &RegistryStamp,
    acquisition_complete: bool,
) -> Vec<ProvenanceRow> {
    let mut rows = Vec::new();
    for metric in &response.metrics {
        if metric.value.is_none() {
            continue;
        }
        let chain = chain_of(response, metric, registry, acquisition_complete);
        rows_for_step(&chain, response, &metric.key, 0, &mut rows);
    }
    rows
}

/// One step of a chain and everything above it, each carrying the depth it sits at.
///
/// The rule row beside the step travels with it and decides nothing. It is consulted for one
/// thing, the spelling of a number the step and the row already agree on, and `rows_for_chain_step`
/// holds it to that agreement.
fn rows_for_step(
    chain: &ProvenanceChain,
    response: &AnalysisResponse,
    quantity: &str,
    depth: usize,
    rows: &mut Vec<ProvenanceRow>,
) {
    let recorded = response
        .bound_methods
        .iter()
        .find(|bound| bound.method_id == chain.provenance.method_id);
    rows.extend(rows_for_chain_step(quantity, chain, recorded, depth));
    for input in &chain.depends_on {
        rows_for_step(input, response, quantity, depth + 1, rows);
    }
}

/// One step of a chain as rows: what the step says produced the number, at the depth it sits.
///
/// Every name, every value and every source is the step's own. A rule's row can add nothing
/// here and can take nothing away, which is the property this relation needs: the tree is what
/// four surfaces publish, and a folder run reading a list beside it would be a fifth account.
/// The derivation already carries values no rule may record, the analysis gravity among them,
/// because no registry entry declares it.
///
/// The row supplies the rule's own spelling of a number, and only where the row holds the
/// number this step is standing on. Rules spell a measured second at four places and a stated
/// one as it was typed, so rendering every value here would rewrite the relation; taking a
/// spelling off a row holding some other number would print a value this step never carried.
///
/// A rule the response named and left no row for still opens the chain, and a step that read
/// nothing still gets a row: dropping either would put the rules above it under nothing.
///
/// Row order is not decided here. `analyse` sorts the whole relation by provenance id,
/// quantity, depth, method id and parameter before writing it, and `provenance_id` sorts its
/// own input before digesting, so an order imposed at this depth reaches neither the file nor
/// the identity.
fn rows_for_chain_step(
    quantity: &str,
    chain: &ProvenanceChain,
    recorded: Option<&BoundMethod>,
    depth: usize,
) -> Vec<ProvenanceRow> {
    let spelling = |name: &str, value: f64| {
        recorded
            .filter(|bound| bound.numeric_values.get(name) == Some(&value))
            .and_then(|bound| {
                bound
                    .bound_parameters
                    .iter()
                    .find(|(held, _)| held == name)
                    .map(|(_, text)| text.clone())
            })
    };

    let mut named: Vec<(String, String, ParameterSource)> = chain
        .provenance
        .parameters
        .iter()
        .map(|record| {
            let text = spelling(&record.name, record.value)
                .unwrap_or_else(|| plateforce_analysis::recorded_number_text(record.value));
            (record.name.clone(), text, record.source)
        })
        .chain(
            chain
                .provenance
                .choices
                .iter()
                .map(|record| (record.name.clone(), record.value.clone(), record.source)),
        )
        .collect();
    // A choice the chain carries beside the step rather than on it. The two hold one set today
    // and the type allows them to differ, so a name reaching only the chain lands here rather
    // than on an account a notebook and an R session publish and this relation does not. Its
    // source is the weakest claim, which is what a record with no recorded source takes
    // everywhere else: nobody wrote down who chose it, so nobody is said to have.
    for (name, value) in &chain.enumerated_choices {
        if !named.iter().any(|(held, _, _)| held == name) {
            named.push((name.clone(), value.clone(), ParameterSource::Assumed));
        }
    }

    if named.is_empty() {
        return vec![ProvenanceRow {
            provenance_id: String::new(),
            quantity: quantity.to_string(),
            depth,
            method_id: chain.provenance.method_id.clone(),
            parameter: String::new(),
            value: String::new(),
            source: String::new(),
        }];
    }
    named
        .into_iter()
        .map(|(parameter, value, source)| ProvenanceRow {
            provenance_id: String::new(),
            quantity: quantity.to_string(),
            depth,
            method_id: chain.provenance.method_id.clone(),
            parameter,
            value,
            // The step recorded where each value came from; deriving it again here could
            // only ever spell two of the six sources.
            source: source.wire_name().to_string(),
        })
        .collect()
}

/// What the folder run publishes for one step of a chain, against the rule row beside it.
///
/// Every case here is a way the step and the row can differ. They do not differ on the
/// committed corpus, which is why this is built rather than read off a run: a guard taken
/// over data where the two agree cannot tell a surface that reads the tree from one that
/// reads the list, and that is the state this relation was in.
#[cfg(test)]
mod rows_come_from_the_chain_step {
    use super::*;
    use plateforce_core::provenance::{ChoiceRecord, ParameterRecord};
    use plateforce_core::Provenance;

    const RULE: &str = "onset.threshold.noise_relative";
    const QUANTITY: &str = "jump_height_from_takeoff_meters";

    fn step(parameters: Vec<ParameterRecord>, choices: Vec<ChoiceRecord>) -> ProvenanceChain {
        ProvenanceChain::leaf(Provenance {
            parameters,
            choices,
            ..Provenance::of(RULE)
        })
    }

    fn number(name: &str, value: f64) -> ParameterRecord {
        ParameterRecord {
            name: name.to_string(),
            value,
            source: ParameterSource::Stated,
        }
    }

    fn choice(name: &str, value: &str) -> ChoiceRecord {
        ChoiceRecord {
            name: name.to_string(),
            value: value.to_string(),
            source: ParameterSource::Stated,
        }
    }

    /// A row for the same rule, holding whatever the caller of this helper says it holds.
    fn row(bound: &[(&str, &str)], numbers: &[(&str, f64)]) -> BoundMethod {
        BoundMethod {
            method_id: RULE.to_string(),
            bound_parameters: bound
                .iter()
                .map(|(name, text)| ((*name).to_string(), (*text).to_string()))
                .collect(),
            parameter_sources: BTreeMap::new(),
            unread_parameters: Vec::new(),
            registry_backed: true,
            placed_by_hand_at_sample: None,
            preset: None,
            method_source: ParameterSource::Stated,
            numeric_values: numbers
                .iter()
                .map(|(name, value)| ((*name).to_string(), *value))
                .collect(),
        }
    }

    fn published(chain: &ProvenanceChain, recorded: Option<&BoundMethod>) -> Vec<(String, String)> {
        rows_for_chain_step(QUANTITY, chain, recorded, 0)
            .into_iter()
            .map(|written| (written.parameter, written.value))
            .collect()
    }

    /// The case the whole entry is about: the chain carries a value the rule may not record,
    /// because no registry entry declares it. The analysis gravity is the live one.
    #[test]
    fn a_value_only_the_step_carries_reaches_the_relation() {
        let chain = step(
            vec![
                number("k", 5.0),
                number("gravity_meters_per_second_squared", 9.80665),
            ],
            Vec::new(),
        );
        let written = published(&chain, Some(&row(&[("k", "5")], &[("k", 5.0)])));

        println!("published: {written:?}");
        assert!(
            written.contains(&(
                "gravity_meters_per_second_squared".to_string(),
                "9.80665".to_string()
            )),
            "the folder run dropped a value the chain carried: {written:?}"
        );
    }

    /// The rule spells the number, and only while it is the same number. A row holding some
    /// other value under the name cannot rewrite what the step stood on.
    #[test]
    fn the_rules_spelling_is_taken_only_where_it_is_the_same_number() {
        let chain = step(vec![number("k", 5.0)], Vec::new());

        let agreeing = published(&chain, Some(&row(&[("k", "5.0")], &[("k", 5.0)])));
        println!("row holds 5.0: {agreeing:?}");
        assert_eq!(
            agreeing,
            vec![("k".to_string(), "5.0".to_string())],
            "the rule's own spelling of its own number was not published"
        );

        let disagreeing = published(&chain, Some(&row(&[("k", "3")], &[("k", 3.0)])));
        println!("row holds 3: {disagreeing:?}");
        assert_eq!(
            disagreeing,
            vec![("k".to_string(), "5".to_string())],
            "a number off the rule's row displaced the one the step ran at"
        );
    }

    /// A choice recorded beside the step rather than on it. `ProvenanceChain::choosing` puts
    /// one there, and the account a reader is shown and the Python package both publish it.
    #[test]
    fn a_choice_carried_beside_the_step_reaches_the_relation() {
        let chain = step(vec![number("k", 5.0)], Vec::new())
            .choosing(vec![("dispersion".to_string(), "sample".to_string())]);
        let written = published(&chain, Some(&row(&[("k", "5")], &[("k", 5.0)])));

        println!("published: {written:?}");
        assert!(
            written.contains(&("dispersion".to_string(), "sample".to_string())),
            "a choice the chain carried reached three surfaces and not this one: {written:?}"
        );
    }

    /// The other side of it, so the merge above cannot be met by writing every name twice.
    #[test]
    fn a_choice_the_step_already_records_is_written_once() {
        let chain = step(Vec::new(), vec![choice("sd_convention", "sample")])
            .choosing(vec![("sd_convention".to_string(), "sample".to_string())]);
        let written = published(&chain, None);

        println!("published: {written:?}");
        assert_eq!(
            written.len(),
            1,
            "one choice was published twice: {written:?}"
        );
        assert_eq!(written[0].1, "sample");
    }

    /// A step that read nothing still gets a row, or the rules above it would sit under
    /// nothing.
    #[test]
    fn a_step_that_read_nothing_still_names_its_rule() {
        let written = rows_for_chain_step(QUANTITY, &step(Vec::new(), Vec::new()), None, 2);
        assert_eq!(written.len(), 1);
        assert_eq!(written[0].method_id, RULE);
        assert_eq!(written[0].depth, 2);
        assert!(written[0].parameter.is_empty());
    }
}
