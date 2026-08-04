//! The flat relations a batch returns.
//!
//! A single table cannot carry a provenance tree and a nested column survives in one
//! language at a time, so the result is flat relations joined on `provenance_id`. Keying
//! provenance on a digest rather than repeating it per trial collapses a corpus-sized run of
//! identical rows to one set, and stays exactly right when one trial ran differently.

use std::collections::BTreeMap;

use plateforce_core::{Acquisition, PlateProfileAttribution};
use serde::{Deserialize, Serialize};

/// One row per trial, one column per quantity. The table people use.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResultRow {
    pub trial_id: String,
    /// Written as walked, and empty when a declared pattern already carries the subject.
    pub source_path: String,
    /// Empty when the trial produced no numbers at all.
    pub provenance_id: String,
    /// Set when the trial produced nothing. A trial that produced some numbers and declined
    /// a landmark carries values here and its refusals in `refusals`.
    pub refusal_code: String,
    pub values: BTreeMap<String, Option<f64>>,
}

impl ResultRow {
    /// The header, with the quantity columns in the order the analysis reported them.
    pub fn header(quantities: &[String]) -> Vec<String> {
        let mut header = vec![
            "trial_id".to_string(),
            "source_path".to_string(),
            "provenance_id".to_string(),
            "refusal_code".to_string(),
        ];
        header.extend(quantities.iter().cloned());
        header
    }

    pub fn cells(&self, quantities: &[String]) -> Vec<String> {
        let mut cells = vec![
            self.trial_id.clone(),
            self.source_path.clone(),
            self.provenance_id.clone(),
            self.refusal_code.clone(),
        ];
        for quantity in quantities {
            cells.push(match self.values.get(quantity) {
                Some(Some(value)) => format_value(*value),
                _ => String::new(),
            });
        }
        cells
    }
}

/// One row per distinct provenance per method per parameter.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProvenanceRow {
    pub provenance_id: String,
    pub quantity: String,
    /// 0 for the arithmetic that produced the quantity, increasing down the chain.
    pub depth: usize,
    pub method_id: String,
    pub parameter: String,
    pub value: String,
    /// stated, assumed or measured, as the fingerprint carries it.
    pub source: String,
}

impl ProvenanceRow {
    pub fn header() -> Vec<String> {
        [
            "provenance_id",
            "quantity",
            "depth",
            "method_id",
            "parameter",
            "value",
            "source",
        ]
        .iter()
        .map(|name| name.to_string())
        .collect()
    }

    pub fn cells(&self) -> Vec<String> {
        vec![
            self.provenance_id.clone(),
            self.quantity.clone(),
            self.depth.to_string(),
            self.method_id.clone(),
            self.parameter.clone(),
            self.value.clone(),
            self.source.clone(),
        ]
    }
}

/// One row per refusal, keyed by trial and ordinal rather than by trial alone.
///
/// A partial trial declines and computes at once, so `trial_id` is not unique here. The
/// seven fields before `trial_id` are the ones a caller branches on; `message` is the
/// sentence the rule that declined already produced, never one composed a second time.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RefusalRow {
    pub trial_id: String,
    pub ordinal: usize,
    pub code: String,
    pub method_id: String,
    pub slot: String,
    pub parameter: String,
    pub value: String,
    pub detail: String,
    pub available: String,
    pub message: String,
}

impl RefusalRow {
    pub fn header() -> Vec<String> {
        [
            "trial_id",
            "ordinal",
            "code",
            "method_id",
            "slot",
            "parameter",
            "value",
            "detail",
            "available",
            "message",
        ]
        .iter()
        .map(|name| name.to_string())
        .collect()
    }

    pub fn cells(&self) -> Vec<String> {
        vec![
            self.trial_id.clone(),
            self.ordinal.to_string(),
            self.code.clone(),
            self.method_id.clone(),
            self.slot.clone(),
            self.parameter.clone(),
            self.value.clone(),
            self.detail.clone(),
            self.available.clone(),
            self.message.clone(),
        ]
    }
}

/// One row per fact the analysis attached to a trial it computed.
///
/// A trial can compute and still carry something the reader needs, so the warned state has a
/// channel of its own. Folding these into `refusals` would say a number was declined when it
/// was produced, and dropping them would lose the state entirely.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WarningRow {
    pub trial_id: String,
    pub ordinal: usize,
    pub message: String,
}

impl WarningRow {
    pub fn header() -> Vec<String> {
        ["trial_id", "ordinal", "message"]
            .iter()
            .map(|name| name.to_string())
            .collect()
    }

    pub fn cells(&self) -> Vec<String> {
        vec![
            self.trial_id.clone(),
            self.ordinal.to_string(),
            self.message.clone(),
        ]
    }
}

/// One row per thing the analysis already knew about a number it reported.
///
/// Keyed by trial and ordinal like `refusals`, because one trial can carry several. Distinct
/// from both neighbours on purpose: a refusal means no number was produced, a warning is a
/// sentence, and a signal qualifies numbers that were produced and carries the fields a reader
/// acts on. Folding it into `warnings` would drop the value, the threshold, the status and the
/// keys it qualifies, which is everything except the prose.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SignalRow {
    pub trial_id: String,
    pub ordinal: usize,
    /// As the record spells it: `disagrees`, `incomparable`, `at_search_floor`.
    pub status: String,
    pub label: String,
    /// Empty where the comparison produced no number, which is a different state from zero.
    pub value: Option<f64>,
    pub unit: String,
    pub threshold: f64,
    /// The quantity columns in `results` this signal is about, comma separated. A reader
    /// joining on `trial_id` alone would not know which of eleven columns it qualifies.
    pub qualifies: String,
    /// The construct whose bound rule the reader would change.
    pub remedy_construct: String,
    /// An action, never a verdict, as the analysis composed it.
    pub remedy: String,
}

impl SignalRow {
    pub fn header() -> Vec<String> {
        [
            "trial_id",
            "ordinal",
            "status",
            "label",
            "value",
            "unit",
            "threshold",
            "qualifies",
            "remedy_construct",
            "remedy",
        ]
        .iter()
        .map(|name| name.to_string())
        .collect()
    }

    pub fn cells(&self) -> Vec<String> {
        vec![
            self.trial_id.clone(),
            self.ordinal.to_string(),
            self.status.clone(),
            self.label.clone(),
            self.value.map(format_value).unwrap_or_default(),
            self.unit.clone(),
            format_value(self.threshold),
            self.qualifies.clone(),
            self.remedy_construct.clone(),
            self.remedy.clone(),
        ]
    }
}

/// One row describing the run. Every count here states the population it was taken over.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunRow {
    pub plateforce_version: String,
    /// The revision the caller pinned, and null when they pinned none. Null rather than
    /// empty: a run row that wrote `""` for an absent pin could not be told apart from one
    /// whose caller pinned the empty string, and `docs/schema.md` settled null for exactly
    /// that reason. Typed `Option` rather than checked at the call site, because a `String`
    /// field has no way to write the value the schema requires.
    pub registry_version: Option<String>,
    /// The revision the registry names about itself, from the `VERSION` file beside its
    /// rules, and null where it names none. What the data claims, never what the caller
    /// cited, and never written into `registry_version`.
    pub registry_declared_version: Option<String>,
    pub registry_digest: String,
    pub request_digest: String,
    /// Names carrying a declared trial suffix. The denominator the file counts are over.
    pub files_found: usize,
    /// Names the run met carrying none of them. Outside `files_found` rather than inside it,
    /// because the declaration removed them before the identity saw them, and a run that
    /// stated only the survivors would be reporting its own narrowing as the folder.
    pub files_without_declared_suffix: usize,
    /// Of those the suffixes kept, the ones the identity could not name. Refused by name,
    /// never skipped.
    pub files_unidentified: usize,
    /// Files the identity named, which is the denominator every trial count is over.
    pub trial_count: usize,
    /// Trials that produced at least one value.
    pub computed_count: usize,
    /// Trials that produced none. A trial that declined one landmark and computed the rest
    /// is counted here as computed, and its declines are rows in `refusals`.
    pub refusal_count: usize,
    pub acquisition_complete_count: usize,
    /// What the plate and its settings were, as the run stated them. Carried whole rather
    /// than as a count, because `run_fingerprint` is taken over this row and the DECREE is
    /// that a fingerprint carries an acquisition block: a row holding only the count would
    /// fingerprint two runs off differently configured plates identically.
    ///
    /// Stated once for the folder, since a trace of forces carries none of it. The members
    /// still missing are `Acquisition::missing`, so a reader is told what to go and find
    /// rather than only that something is absent.
    pub acquisition: Acquisition,
    /// Whether the block above holds every member. `acquisition_complete_count` is the same
    /// fact multiplied by the trials it applied to; this is the fact itself, and it is the
    /// one a reader comparing two runs asks. A run whose block is incomplete must never be
    /// declared to match another, whatever the two digests read.
    pub acquisition_complete: bool,
    /// The saved plate the block above was filled from, absent when the caller typed the
    /// members or stated none.
    ///
    /// Outside `run_fingerprint` on purpose, which is why `run_fingerprint` clears it rather
    /// than reading the whole row: what a lab calls its own plate is not a fact about the
    /// capture, and two labs whose plates are configured alike have to match whatever names
    /// they file them under.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plate_profile: Option<PlateProfileAttribution>,
    pub trials_excluded: usize,
    pub gates_reporting: usize,
    pub gates_applied: usize,
    /// Distinct chains across the run. One means every trial was analysed by the same rules
    /// under the same parameters and produced the same quantities, which is the question a
    /// reader has to answer before pooling and which is otherwise a diff rather than a query.
    pub distinct_provenance_count: usize,
    pub trial_identity: String,
    /// How every file in the run was read. A request digest says two runs differ; these say
    /// how, and three of the four are stated by the caller and were otherwise lost.
    pub delimiter: String,
    pub force_column_index: usize,
    pub sample_rate_hz: f64,
    /// The value the run read as missing, or empty when the caller declared none. Empty is a
    /// declaration here, because the format field it comes from has no default.
    pub sentinel: String,
    /// Samples the declared sentinel removed, across every trial the run read.
    pub sentinel_rows_dropped: usize,
    /// The digest over this row and the distinct chains it held, and null when the
    /// acquisition block was not filled.
    ///
    /// Null rather than a marked digest. A run whose plate settings nobody recorded cannot be
    /// declared to match another, and a value that string-compares equal to the next such run
    /// is that declaration whatever it is called. `acquisition_complete` above says why it is
    /// null, and `acquisition.missing` names what would fill it.
    pub run_fingerprint: Option<String>,
}

impl RunRow {
    /// Three arithmetic statements the run makes about itself. A run that breaks one has
    /// broken an invariant it stated, which is a different fault from a rule declining.
    pub fn check_invariants(&self) -> Result<(), String> {
        if self.files_found != self.trial_count + self.files_unidentified {
            return Err(format!(
                "files_found {} does not equal named {} plus unidentified {}",
                self.files_found, self.trial_count, self.files_unidentified
            ));
        }
        if self.trial_count != self.computed_count + self.refusal_count {
            return Err(format!(
                "trial_count {} does not equal computed {} plus refused {}",
                self.trial_count, self.computed_count, self.refusal_count
            ));
        }
        if self.trials_excluded > self.computed_count {
            return Err(format!(
                "trials_excluded {} of {} exceeds the {} trials that computed",
                self.trials_excluded, self.trial_count, self.computed_count
            ));
        }
        Ok(())
    }
}

/// One row per group per quantity, written only when the request bound an aggregation rule.
///
/// A mean row inside `results` would put a row in it that is not a trial and break the
/// relation, so the reduction is a fifth relation and `results` is untouched.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AggregateRow {
    pub group_key: String,
    /// subject, session or run.
    pub group_kind: String,
    pub quantity: String,
    pub value: Option<f64>,
    pub dispersion: Option<f64>,
    /// Travels with every aggregated value, because best of five and best of three are
    /// different numbers.
    pub n: usize,
    /// Empty for a run-level reduction, which no registry entry publishes a rule for.
    pub method_id: String,
    pub provenance_id: String,
}

impl AggregateRow {
    pub fn header() -> Vec<String> {
        [
            "group_key",
            "group_kind",
            "quantity",
            "value",
            "dispersion",
            "n",
            "method_id",
            "provenance_id",
        ]
        .iter()
        .map(|name| name.to_string())
        .collect()
    }

    pub fn cells(&self) -> Vec<String> {
        vec![
            self.group_key.clone(),
            self.group_kind.clone(),
            self.quantity.clone(),
            self.value.map(format_value).unwrap_or_default(),
            self.dispersion.map(format_value).unwrap_or_default(),
            self.n.to_string(),
            self.method_id.clone(),
            self.provenance_id.clone(),
        ]
    }
}

/// Full precision, because a table written at display precision is a different number from
/// the one the rule produced and the two get compared.
pub(crate) fn format_value(value: f64) -> String {
    if value == value.trunc() && value.abs() < 1e15 {
        format!("{value:.1}")
    } else {
        format!("{value}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_result_header_carries_the_four_keys_then_the_quantities_in_order() {
        let quantities = vec![
            "jump_height_from_takeoff_meters".to_string(),
            "takeoff_velocity_meters_per_second".to_string(),
        ];
        assert_eq!(
            ResultRow::header(&quantities),
            vec![
                "trial_id",
                "source_path",
                "provenance_id",
                "refusal_code",
                "jump_height_from_takeoff_meters",
                "takeoff_velocity_meters_per_second",
            ]
        );
    }

    #[test]
    fn a_run_that_named_more_trials_than_it_placed_fails_its_own_invariant() {
        let mut run = run_fixture();
        run.files_found = 7;
        run.trial_count = 7;
        run.computed_count = 5;
        run.refusal_count = 1;
        let error = run.check_invariants().unwrap_err();
        assert!(error.contains("trial_count 7"), "{error}");
        assert!(error.contains("computed 5"), "{error}");
    }

    #[test]
    fn a_file_that_left_the_denominator_without_being_named_fails_its_own_invariant() {
        let mut run = run_fixture();
        run.files_found = 8;
        let error = run.check_invariants().unwrap_err();
        assert!(error.contains("files_found 8"), "{error}");
        assert!(error.contains("named 6"), "{error}");
    }

    #[test]
    fn excluding_more_trials_than_computed_fails_its_own_invariant() {
        let mut run = run_fixture();
        run.trials_excluded = 9;
        let error = run.check_invariants().unwrap_err();
        assert!(error.contains("9 of 6"), "{error}");
    }

    /// A file the declared suffixes passed over is counted and stays outside the denominator
    /// the named trials are taken over. Folding it in would say the identity failed to name a
    /// file nothing ever asked it to name.
    #[test]
    fn files_carrying_no_declared_suffix_are_counted_outside_the_named_denominator() {
        let mut run = run_fixture();
        run.files_without_declared_suffix = 3;
        assert!(run.check_invariants().is_ok(), "{run:?}");

        run.files_found += 3;
        let error = run
            .check_invariants()
            .expect_err("a file the suffixes passed over is not a file the identity named");
        assert!(error.contains("files_found 9"), "{error}");
        assert!(error.contains("named 6"), "{error}");
    }

    fn run_fixture() -> RunRow {
        RunRow {
            plateforce_version: "0.1.0".to_string(),
            registry_version: None,
            registry_declared_version: None,
            registry_digest: "content-0".to_string(),
            request_digest: "content-1".to_string(),
            files_found: 6,
            files_without_declared_suffix: 0,
            files_unidentified: 0,
            trial_count: 6,
            computed_count: 6,
            refusal_count: 0,
            acquisition_complete_count: 0,
            acquisition: Acquisition::default(),
            acquisition_complete: false,
            plate_profile: None,
            trials_excluded: 0,
            gates_reporting: 0,
            gates_applied: 0,
            distinct_provenance_count: 1,
            trial_identity: "file_stem".to_string(),
            delimiter: "\t".to_string(),
            force_column_index: 0,
            sample_rate_hz: 1200.0,
            sentinel: String::new(),
            sentinel_rows_dropped: 0,
            run_fingerprint: None,
        }
    }
}
