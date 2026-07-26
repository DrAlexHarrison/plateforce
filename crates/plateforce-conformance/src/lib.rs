//! Conformance of the Rust core against a frozen reference run.
//!
//! The reference is a numpy implementation of 56 method variants over 244 real
//! countermovement jumps, each rule cited to the upstream file and line it reproduces.
//! Its output is frozen as a fixture and never regenerated as part of a test run, so a
//! change to the Rust that moves a number has to be explained rather than absorbed.
//!
//! This is not a second implementation of the math. Nothing in this crate computes a
//! force-plate quantity; it calls `plateforce-core` and compares.

pub mod bindings;
pub mod corpus;

use bindings::{
    analyse, ReferenceBindings, TrialAnalysis, TrialRejection, ONSET_RULES, REFERENCE_NOT_FOUND,
    TAKEOFF_RULES, UNWEIGHTING_END_RULES,
};
use plateforce_core::signal::{partition_sentinels, Sentinel};
use plateforce_core::Trial;
use std::collections::HashMap;

/// How close two numbers have to be to count as the same number.
///
/// The relative floor is set from the measured spread rather than chosen: with
/// compensated summation on the Rust side every float column agrees to better than
/// 1e-12 relative, so 1e-9 leaves three orders of headroom for a compiler or platform
/// difference while still being eleven orders tighter than the smallest physically
/// meaningful difference in any of these quantities.
#[derive(Debug, Clone, Copy)]
pub struct Tolerance {
    pub relative: f64,
    pub absolute: f64,
}

impl Default for Tolerance {
    fn default() -> Self {
        Self {
            relative: 1e-9,
            absolute: 1e-12,
        }
    }
}

impl Tolerance {
    fn agrees(&self, left: f64, right: f64) -> bool {
        if left.is_nan() && right.is_nan() {
            return true;
        }
        if left.is_nan() != right.is_nan() {
            return false;
        }
        let difference = (left - right).abs();
        difference <= self.absolute || difference <= self.relative * left.abs().max(right.abs())
    }
}

/// Whether a column is an index or a measurement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Agreement {
    /// Indices and counts. A one sample difference is a different answer, not a
    /// rounding difference, so nothing is forgiven here.
    Exact,
    Numeric,
}

#[derive(Debug, Clone)]
pub struct ColumnResult {
    pub name: String,
    pub agreement: Agreement,
    pub compared: usize,
    pub matched: usize,
    pub worst_absolute_difference: f64,
    pub worst_relative_difference: f64,
    pub worst_trial: Option<(u32, u32)>,
    pub reference_value: f64,
    pub computed_value: f64,
    /// Every trial that disagreed, so a breach can be investigated without a rerun.
    pub disagreements: Vec<Disagreement>,
}

#[derive(Debug, Clone, Copy)]
pub struct Disagreement {
    pub subject: u32,
    pub trial: u32,
    pub reference: f64,
    pub computed: f64,
}

impl ColumnResult {
    pub fn is_clean(&self) -> bool {
        self.compared == self.matched
    }
}

/// A trial the harness refused to compare, and the rule it refused under.
#[derive(Debug, Clone)]
pub struct Exclusion {
    pub subject: u32,
    pub trial: u32,
    pub reason: String,
}

#[derive(Debug, Clone, Default)]
pub struct ConformanceReport {
    pub columns: Vec<ColumnResult>,
    pub trials_compared: usize,
    pub reference_rows: usize,
    pub exclusions: Vec<Exclusion>,
    pub missing_from_corpus: Vec<(u32, u32)>,
    pub tied_weighing_window_trials: usize,
    pub worst_tied_weighing_window_count: usize,
    pub late_takeoff_trials: usize,
}

impl ConformanceReport {
    pub fn breaches(&self) -> Vec<&ColumnResult> {
        self.columns.iter().filter(|c| !c.is_clean()).collect()
    }

    pub fn is_clean(&self) -> bool {
        self.breaches().is_empty() && self.missing_from_corpus.is_empty()
    }
}

/// One row of the frozen reference output.
#[derive(Debug, Clone)]
pub struct ReferenceRow {
    pub subject: u32,
    pub trial: u32,
    values: HashMap<String, f64>,
}

impl ReferenceRow {
    pub fn get(&self, column: &str) -> Option<f64> {
        self.values.get(column).copied()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ConformanceError {
    #[error("reference output has no header row")]
    NoHeader,
    #[error("reference output row {row} has {found} fields against {expected} in the header")]
    RaggedRow {
        row: usize,
        found: usize,
        expected: usize,
    },
    #[error("reference output is missing the {0} column, which identifies the trial")]
    MissingIdentifier(&'static str),
    #[error("reading {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
}

/// Parse the frozen reference output. Fields are plain numbers with no quoting, and a
/// value the reference could not compute is written as `nan`.
pub fn parse_reference(text: &str) -> Result<Vec<ReferenceRow>, ConformanceError> {
    let mut lines = text.lines().filter(|line| !line.trim().is_empty());
    let header: Vec<&str> = lines.next().ok_or(ConformanceError::NoHeader)?.split(',').collect();
    let subject_at = header
        .iter()
        .position(|name| *name == "subject")
        .ok_or(ConformanceError::MissingIdentifier("subject"))?;
    let trial_at = header
        .iter()
        .position(|name| *name == "trial")
        .ok_or(ConformanceError::MissingIdentifier("trial"))?;

    let mut rows = Vec::new();
    for (offset, line) in lines.enumerate() {
        let fields: Vec<&str> = line.split(',').collect();
        if fields.len() != header.len() {
            return Err(ConformanceError::RaggedRow {
                row: offset + 2,
                found: fields.len(),
                expected: header.len(),
            });
        }
        let mut values = HashMap::with_capacity(header.len());
        for (name, field) in header.iter().zip(&fields) {
            values.insert((*name).to_string(), parse_number(field));
        }
        rows.push(ReferenceRow {
            subject: fields[subject_at].trim().parse().unwrap_or(0),
            trial: fields[trial_at].trim().parse().unwrap_or(0),
            values,
        });
    }
    Ok(rows)
}

fn parse_number(field: &str) -> f64 {
    let trimmed = field.trim();
    if trimmed.is_empty() {
        return f64::NAN;
    }
    trimmed.parse().unwrap_or(f64::NAN)
}

fn index_as_number(index: Option<usize>) -> f64 {
    index.map(|value| value as f64).unwrap_or(REFERENCE_NOT_FOUND)
}

fn seconds_or_missing(index: Option<usize>, sample_rate_hz: f64) -> f64 {
    match index {
        Some(value) if value > 0 => value as f64 / sample_rate_hz,
        _ => f64::NAN,
    }
}

/// Flatten one analysis into the reference's column names.
pub fn computed_columns(
    analysis: &TrialAnalysis,
    bindings: &ReferenceBindings,
) -> Vec<(String, f64, Agreement)> {
    let rate = bindings.sample_rate_hz;
    let mut columns: Vec<(String, f64, Agreement)> = vec![
        ("bw_025".into(), analysis.system_weight_newtons[0], Agreement::Numeric),
        ("bw_040".into(), analysis.system_weight_newtons[1], Agreement::Numeric),
        ("bw_100".into(), analysis.system_weight_newtons[2], Agreement::Numeric),
        ("bw_med".into(), analysis.median_weight_newtons, Agreement::Numeric),
        ("bw_lowvar".into(), analysis.lowest_variance_weight_newtons, Agreement::Numeric),
        ("sd_025".into(), analysis.standard_deviation_newtons[0], Agreement::Numeric),
        ("sd_050".into(), analysis.standard_deviation_newtons[1], Agreement::Numeric),
    ];

    for (slot, rule) in ONSET_RULES.iter().enumerate() {
        columns.push((
            format!("onset_{rule}"),
            index_as_number(analysis.onset_index[slot]),
            Agreement::Exact,
        ));
        columns.push((
            format!("ttt_{rule}"),
            analysis.time_to_takeoff_seconds(slot, rate),
            Agreement::Numeric,
        ));
        columns.push((
            format!("jh_{rule}_cm"),
            analysis.jump_height_centimetres[slot],
            Agreement::Numeric,
        ));
    }
    for (slot, rule) in TAKEOFF_RULES.iter().enumerate() {
        columns.push((
            format!("takeoff_{rule}"),
            index_as_number(analysis.takeoff_index[slot]),
            Agreement::Exact,
        ));
    }
    for (slot, rule) in UNWEIGHTING_END_RULES.iter().enumerate() {
        columns.push((
            format!("uwend_{rule}"),
            index_as_number(analysis.unweighting_end_index[slot]),
            Agreement::Exact,
        ));
        columns.push((
            format!("uwend_{rule}_s"),
            seconds_or_missing(analysis.unweighting_end_index[slot], rate),
            Agreement::Numeric,
        ));
    }
    columns.push((
        "n_lowforce_runs".into(),
        analysis.qualifying_low_force_run_count as f64,
        Agreement::Exact,
    ));
    columns.push((
        "longest_run_is_first".into(),
        f64::from(u8::from(analysis.longest_run_is_first_qualifying)),
        Agreement::Exact,
    ));
    columns.push((
        "velocity_zero".into(),
        index_as_number(analysis.velocity_zero_index),
        Agreement::Exact,
    ));
    columns.push((
        "min_force_before_peak".into(),
        index_as_number(analysis.force_minimum_before_peak),
        Agreement::Exact,
    ));
    columns.push((
        "min_force_before_takeoff".into(),
        index_as_number(analysis.force_minimum_before_takeoff),
        Agreement::Exact,
    ));
    columns.push((
        "jh_jm_wholewindow_cm".into(),
        analysis.whole_window_jump_height_centimetres,
        Agreement::Numeric,
    ));
    columns
}

/// Columns the reference carries from a commercial export rather than computing, so
/// there is nothing for the Rust to conform to. They are still read, because the
/// missing-data sentinel lives in them.
pub const VENDOR_COLUMNS: [&str; 3] = ["ttt_accupower", "jh_accupower_cm", "rsi_accupower"];

/// Run every reference row against the Rust core.
pub fn compare(
    reference: &[ReferenceRow],
    corpus: &dyn Fn(u32, u32) -> Option<Trial>,
    bindings: &ReferenceBindings,
    tolerance: Tolerance,
) -> ConformanceReport {
    let mut report = ConformanceReport {
        reference_rows: reference.len(),
        ..Default::default()
    };
    let mut accumulated: Vec<ColumnResult> = Vec::new();
    let mut column_at: HashMap<String, usize> = HashMap::new();

    for row in reference {
        // A commercial export writes 0.00 to height, time and index together when it
        // has no measurement. A countermovement jump of 0.00 cm is not a small jump.
        let vendor: Vec<f64> = VENDOR_COLUMNS
            .iter()
            .map(|name| row.get(name).unwrap_or(f64::NAN))
            .collect();
        let (_, sentinel_positions) = partition_sentinels(&vendor, Sentinel::Zero);
        if sentinel_positions.len() == VENDOR_COLUMNS.len() {
            report.exclusions.push(Exclusion {
                subject: row.subject,
                trial: row.trial,
                reason: format!(
                    "vendor sentinel: {} written to {} simultaneously",
                    Sentinel::Zero.describe(),
                    VENDOR_COLUMNS.join(", ")
                ),
            });
        }

        let Some(trial) = corpus(row.subject, row.trial) else {
            report.missing_from_corpus.push((row.subject, row.trial));
            continue;
        };
        let analysis = match analyse(&trial, bindings) {
            Ok(analysis) => analysis,
            Err(rejection) => {
                report.exclusions.push(Exclusion {
                    subject: row.subject,
                    trial: row.trial,
                    reason: match rejection {
                        TrialRejection::TooShort { samples, required } => {
                            format!("trace of {samples} samples is shorter than the required {required}")
                        }
                        TrialRejection::NoTakeoff => "no takeoff found".to_string(),
                    },
                });
                continue;
            }
        };
        report.trials_compared += 1;
        if analysis.lowest_variance_tied_window_count > 1 {
            report.tied_weighing_window_trials += 1;
            report.worst_tied_weighing_window_count = report
                .worst_tied_weighing_window_count
                .max(analysis.lowest_variance_tied_window_count);
        }
        if !analysis.longest_run_is_first_qualifying {
            report.late_takeoff_trials += 1;
        }

        for (name, computed, agreement) in computed_columns(&analysis, bindings) {
            let Some(expected) = row.get(&name) else {
                continue;
            };
            let slot = *column_at.entry(name.clone()).or_insert_with(|| {
                accumulated.push(ColumnResult {
                    name: name.clone(),
                    agreement,
                    compared: 0,
                    matched: 0,
                    worst_absolute_difference: 0.0,
                    worst_relative_difference: 0.0,
                    worst_trial: None,
                    reference_value: f64::NAN,
                    computed_value: f64::NAN,
                    disagreements: Vec::new(),
                });
                accumulated.len() - 1
            });
            let result = &mut accumulated[slot];
            result.compared += 1;

            let agrees = match agreement {
                Agreement::Exact => {
                    (expected.is_nan() && computed.is_nan()) || expected == computed
                }
                Agreement::Numeric => tolerance.agrees(expected, computed),
            };
            if agrees {
                result.matched += 1;
            } else {
                result.disagreements.push(Disagreement {
                    subject: row.subject,
                    trial: row.trial,
                    reference: expected,
                    computed,
                });
            }
            let difference = if expected.is_nan() && computed.is_nan() {
                0.0
            } else if expected.is_nan() != computed.is_nan() {
                f64::INFINITY
            } else {
                (expected - computed).abs()
            };
            if difference > result.worst_absolute_difference || (!agrees && result.worst_trial.is_none())
            {
                result.worst_absolute_difference = difference;
                result.worst_relative_difference =
                    difference / expected.abs().max(computed.abs()).max(f64::MIN_POSITIVE);
                result.worst_trial = Some((row.subject, row.trial));
                result.reference_value = expected;
                result.computed_value = computed;
            }
        }
    }

    accumulated.sort_by(|left, right| left.name.cmp(&right.name));
    report.columns = accumulated;
    report
}

trait SentinelDescription {
    fn describe(&self) -> String;
}

impl SentinelDescription for Sentinel {
    fn describe(&self) -> String {
        match self {
            Sentinel::Zero => "0.00".to_string(),
            Sentinel::NegativeOne => "-1".to_string(),
            Sentinel::Value(value) => value.to_string(),
        }
    }
}
