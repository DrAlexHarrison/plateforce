//! Every answer this crate gives R is a JSON document, and every one is either
//! `{"ok": ...}` or `{"refusal": ...}`.
//!
//! A refusal is data rather than a raised error because neither R binding framework can
//! construct a classed R condition carrying fields, and the R surface owes its caller a
//! condition with seven of them. Carrying it as data makes that one rule instead of two,
//! and it means R receives the same bytes a cross-surface comparison reads.
//!
//! The force trace does not travel this way. It is thousands of doubles, JSON round-trips
//! it neither cheaply nor exactly, and R doubles are already IEEE 754 binary64.

pub mod shim;

use std::collections::BTreeMap;

use plateforce_analysis::spread::{self, SpreadRequest, SpreadResponse};
use plateforce_analysis::{run, AnalysisRequest, AnalysisResponse, Binding, BINDINGS};
use plateforce_core::read::read_delimited_column;
use plateforce_core::reporting::describe;
use plateforce_core::signal::partition_sentinels;
use plateforce_core::{Measured, Provenance, ProvenanceChain, Sentinel, Trial};
use plateforce_registry::{Method, Registry};
use serde::{Deserialize, Serialize};

/// The fields an R condition carries, named as the cross-surface idiom names them, so the
/// R class vector is `paste0("plateforce_", code)` with no translation table in between.
#[derive(Debug, Clone, Serialize)]
pub struct Refusal {
    pub code: String,
    pub message: String,
    pub method_id: Option<String>,
    pub slot: Option<String>,
    pub parameter: Option<String>,
    pub value: Option<String>,
    pub detail: Option<String>,
    pub available: Option<Vec<String>>,
}

impl Refusal {
    fn of(code: &str, message: String) -> Self {
        Self {
            code: code.to_string(),
            message,
            method_id: None,
            slot: None,
            parameter: None,
            value: None,
            detail: None,
            available: None,
        }
    }

    fn naming_parameter(code: &str, parameter: &str, message: String) -> Self {
        Self {
            parameter: Some(parameter.to_string()),
            ..Self::of(code, message)
        }
    }

    fn about(mut self, value: impl Into<String>) -> Self {
        self.value = Some(value.into());
        self
    }
}

#[derive(Serialize)]
#[serde(untagged)]
enum Envelope<T: Serialize> {
    Ok { ok: T },
    Refused { refusal: Refusal },
}

fn ok<T: Serialize>(value: T) -> String {
    encode(Envelope::Ok { ok: value })
}

fn refuse<T: Serialize>(refusal: Refusal) -> String {
    encode(Envelope::<T>::Refused { refusal })
}

/// A serialisation that itself failed would otherwise leave the caller holding nothing, so
/// it is reported in the shape every other answer arrives in.
fn encode<T: Serialize>(envelope: Envelope<T>) -> String {
    match serde_json::to_string(&envelope) {
        Ok(text) => text,
        Err(error) => format!(
            "{{\"refusal\":{{\"code\":\"serialisation_failed\",\"message\":{},\"method_id\":null,\"slot\":null,\"parameter\":null,\"value\":null,\"detail\":null,\"available\":null}}}}",
            serde_json::to_string(&error.to_string()).unwrap_or_else(|_| "\"\"".to_string())
        ),
    }
}

fn parse_request<T: serde::de::DeserializeOwned>(request_json: &str) -> Result<T, Box<Refusal>> {
    serde_json::from_str(request_json).map_err(|error| {
        Box::new(Refusal::of(
            "request_not_understood",
            format!("the request could not be read: {error}"),
        ))
    })
}

#[derive(Serialize)]
struct Version {
    package_version: &'static str,
}

pub fn version_json() -> String {
    ok(Version {
        package_version: env!("CARGO_PKG_VERSION"),
    })
}

#[derive(Serialize)]
struct CensusRow {
    population: String,
    count: usize,
    genuine_debates: Option<usize>,
    can_find_wrong_event: Option<usize>,
}

#[derive(Serialize)]
struct RegistryReport {
    root: String,
    digest: String,
    census: Vec<CensusRow>,
    method_ids: Vec<String>,
    construct_ids: Vec<String>,
    protocol_ids: Vec<String>,
}

/// The census, one row per population, each derived count beside the denominator it was
/// taken over. No total row and no total column: the two populations are not one
/// population, and a table carrying a total invites adding them.
pub fn registry_json(root: &str) -> String {
    let registry = match Registry::load(root) {
        Ok(registry) => registry,
        Err(error) => {
            return refuse::<RegistryReport>(Refusal::of("registry_unreadable", error.to_string()))
        }
    };
    let census = registry.census();
    ok(RegistryReport {
        root: root.to_string(),
        digest: registry.content_digest.clone(),
        census: vec![
            CensusRow {
                population: "constructs".to_string(),
                count: census.constructs,
                genuine_debates: None,
                can_find_wrong_event: None,
            },
            CensusRow {
                population: "computation_entries".to_string(),
                count: census.computation_entries,
                genuine_debates: Some(registry.genuine_debates().count()),
                can_find_wrong_event: Some(registry.methods_that_can_fail().count()),
            },
            CensusRow {
                population: "protocol_entries".to_string(),
                count: census.protocol_entries,
                genuine_debates: None,
                can_find_wrong_event: None,
            },
        ],
        method_ids: registry.methods.keys().cloned().collect(),
        construct_ids: registry.constructs.keys().cloned().collect(),
        protocol_ids: registry.protocols.keys().cloned().collect(),
    })
}

/// One entry, serialised through the registry's own schema types, so no field is dropped
/// on the way to R and no second spelling of a field name exists here.
pub fn registry_entry_json(root: &str, id: &str) -> String {
    let registry = match Registry::load(root) {
        Ok(registry) => registry,
        Err(error) => {
            return refuse::<Method>(Refusal::of("registry_unreadable", error.to_string()))
        }
    };
    match registry.methods.get(id) {
        Some(method) => ok(method.clone()),
        None => refuse::<Method>(Refusal {
            method_id: Some(id.to_string()),
            ..Refusal::of(
                "method_not_in_registry",
                format!("no entry in this registry has the id {id}"),
            )
        }),
    }
}

/// Every rule this build can run, straight off the binding table, so a caller lists the
/// alternatives from the engine rather than from a list written a second time in R.
pub fn bindings_json() -> String {
    ok(BINDINGS.iter().collect::<Vec<&Binding>>())
}

/// One trace, held on the Rust side for the life of the R object pointing at it.
pub struct TrialHandle {
    trial: Trial,
    report: TrialReport,
}

/// What the reader did, rather than what it assumed. Both silent exclusion and silent
/// inclusion are reportable here, and a reader is where both happen.
#[derive(Serialize, Clone)]
pub struct TrialReport {
    sample_count: usize,
    sample_rate_hz: f64,
    duration_seconds: f64,
    sentinel_convention: String,
    samples_treated_as_missing: usize,
    source: String,
    delimiter: Option<String>,
    force_column: Option<usize>,
    rows_read: Option<usize>,
    columns_per_row: Option<usize>,
    blank_lines_skipped: Option<usize>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TrialRequest {
    sample_rate_hz: Option<f64>,
    #[serde(default)]
    sentinel_convention: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReadRequest {
    path: String,
    sample_rate_hz: Option<f64>,
    delimiter: Option<String>,
    force_column: Option<usize>,
    #[serde(default)]
    sentinel_convention: Option<String>,
}

fn sentinel_from(convention: &str) -> Result<Option<Sentinel>, Box<Refusal>> {
    match convention {
        "none" => Ok(None),
        "zero" => Ok(Some(Sentinel::Zero)),
        "negative_one" => Ok(Some(Sentinel::NegativeOne)),
        other => Err(Box::new(Refusal {
            available: Some(vec![
                "none".to_string(),
                "zero".to_string(),
                "negative_one".to_string(),
            ]),
            ..Refusal::naming_parameter(
                "unknown_sentinel_convention",
                "sentinel_convention",
                format!("{other} is not a sentinel convention this reader applies"),
            )
            .about(other)
        })),
    }
}

/// A sample matching the declared convention is held at the last real reading and counted.
/// Closing the gap instead would shift every timestamp after it.
fn apply_sentinel(values: &[f64], sentinel: Option<Sentinel>) -> (Vec<f64>, usize) {
    let flagged = sentinel.map(|convention| partition_sentinels(values, convention).1);
    let mut held = 0usize;
    let mut force = Vec::with_capacity(values.len());
    for (index, value) in values.iter().enumerate() {
        let missing = !value.is_finite()
            || flagged
                .as_ref()
                .is_some_and(|dropped| dropped.binary_search(&index).is_ok());
        if missing {
            held += 1;
            force.push(force.last().copied().unwrap_or(0.0));
        } else {
            force.push(*value);
        }
    }
    (force, held)
}

fn build_trial(
    values: &[f64],
    sample_rate_hz: Option<f64>,
    convention: Option<String>,
    source: String,
) -> Result<TrialHandle, Box<Refusal>> {
    let sample_rate_hz = sample_rate_hz.ok_or_else(|| {
        Box::new(Refusal::naming_parameter(
            "sample_rate_not_declared",
            "sample_rate_hz",
            "this trace carries no sample rate, so state the rate it was recorded at".to_string(),
        ))
    })?;
    let convention = convention.unwrap_or_else(|| "none".to_string());
    let sentinel = sentinel_from(&convention)?;
    let (force, held) = apply_sentinel(values, sentinel);
    let trial = Trial::new(force, sample_rate_hz)
        .map_err(|error| Box::new(Refusal::of("trace_unusable", error.to_string())))?;
    let report = TrialReport {
        sample_count: trial.len(),
        sample_rate_hz: trial.sample_rate_hz(),
        duration_seconds: trial.duration_seconds(),
        sentinel_convention: convention,
        samples_treated_as_missing: held,
        source,
        delimiter: None,
        force_column: None,
        rows_read: None,
        columns_per_row: None,
        blank_lines_skipped: None,
    };
    Ok(TrialHandle { trial, report })
}

pub fn trial_from_force(
    force_newtons: &[f64],
    request_json: &str,
) -> (String, Option<TrialHandle>) {
    let request: TrialRequest = match parse_request(request_json) {
        Ok(request) => request,
        Err(refusal) => return (refuse::<TrialReport>(*refusal), None),
    };
    match build_trial(
        force_newtons,
        request.sample_rate_hz,
        request.sentinel_convention,
        "vector".to_string(),
    ) {
        Ok(handle) => (ok(handle.report.clone()), Some(handle)),
        Err(refusal) => (refuse::<TrialReport>(*refusal), None),
    }
}

/// The file is read by the engine. The caller states the delimiter and the force column
/// rather than having either inferred: which column carries vertical ground reaction force
/// is a property of the export, and a reader that guesses it can be wrong quietly.
pub fn trial_from_file(request_json: &str) -> (String, Option<TrialHandle>) {
    let request: ReadRequest = match parse_request(request_json) {
        Ok(request) => request,
        Err(refusal) => return (refuse::<TrialReport>(*refusal), None),
    };

    let delimiter = match declared_delimiter(request.delimiter.as_deref()) {
        Ok(delimiter) => delimiter,
        Err(refusal) => return (refuse::<TrialReport>(*refusal), None),
    };
    let force_column = match request.force_column {
        Some(column) => column,
        None => {
            return (
                refuse::<TrialReport>(Refusal::naming_parameter(
                    "force_column_not_declared",
                    "force_column",
                    "name the column that carries vertical ground reaction force".to_string(),
                )),
                None,
            )
        }
    };

    let text = match std::fs::read_to_string(&request.path) {
        Ok(text) => text,
        Err(error) => {
            return (
                refuse::<TrialReport>(
                    Refusal::of("file_unreadable", error.to_string()).about(request.path.clone()),
                ),
                None,
            )
        }
    };

    let (values, column) = match read_delimited_column(&text, delimiter, force_column) {
        Ok(read) => read,
        Err(error) => {
            return (
                refuse::<TrialReport>(Refusal::of("file_not_read", error.to_string())),
                None,
            )
        }
    };

    match build_trial(
        &values,
        request.sample_rate_hz,
        request.sentinel_convention,
        request.path.clone(),
    ) {
        Ok(mut handle) => {
            handle.report.delimiter = Some(delimiter.to_string());
            handle.report.force_column = Some(column.column_index);
            handle.report.rows_read = Some(column.rows_read);
            handle.report.columns_per_row = Some(column.columns_per_row);
            handle.report.blank_lines_skipped = Some(column.blank_lines_skipped);
            (ok(handle.report.clone()), Some(handle))
        }
        Err(refusal) => (refuse::<TrialReport>(*refusal), None),
    }
}

fn declared_delimiter(declared: Option<&str>) -> Result<char, Box<Refusal>> {
    let text = declared.ok_or_else(|| {
        Box::new(Refusal::naming_parameter(
            "delimiter_not_declared",
            "delimiter",
            "state the character that separates this file's columns".to_string(),
        ))
    })?;
    let mut characters = text.chars();
    match (characters.next(), characters.next()) {
        (Some(single), None) => Ok(single),
        _ => Err(Box::new(
            Refusal::naming_parameter(
                "delimiter_not_one_character",
                "delimiter",
                "a column separator is one character".to_string(),
            )
            .about(text),
        )),
    }
}

pub fn trial_report_json(handle: &TrialHandle) -> String {
    ok(handle.report.clone())
}

/// A trial saved to disk and reloaded comes back as a pointer to nothing, and the R side
/// has to be able to say so rather than analyse an empty trace.
pub fn handle_lost_json() -> String {
    refuse::<TrialReport>(Refusal::of(
        "trial_not_in_this_session",
        "this trial belongs to a session that has ended, so read the trace again".to_string(),
    ))
}

pub fn trial_force(handle: &TrialHandle) -> &[f64] {
    handle.trial.force()
}

#[derive(Serialize)]
struct AnalysisReport {
    #[serde(flatten)]
    response: AnalysisResponse,
    registry_digest: Option<String>,
    acquisition_complete: bool,
    /// The account each quantity gives of itself, keyed by the quantity. Generated by the
    /// engine so a number reads the same in a notebook, a browser tab and an R session.
    descriptions: BTreeMap<String, String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AnalyseRequest {
    #[serde(flatten)]
    analysis: AnalysisRequest,
    /// Measured by the caller from the registry it loaded, rather than measured again
    /// here. Loading and validating the registry costs nine milliseconds and this path
    /// does not read a rule out of it.
    registry_digest: Option<String>,
}

pub fn analyse_json(handle: &TrialHandle, request_json: &str) -> String {
    let request: AnalyseRequest = match parse_request(request_json) {
        Ok(request) => request,
        Err(refusal) => return refuse::<AnalysisReport>(*refusal),
    };

    match run(&handle.trial, &request.analysis) {
        Ok(response) => ok(AnalysisReport {
            descriptions: descriptions_of(&response, &request.registry_digest),
            response,
            registry_digest: request.registry_digest,
            // No acquisition block reaches this binding yet, and a dataset that cannot
            // fill one fingerprints as incomplete rather than as matching.
            acquisition_complete: false,
        }),
        Err(message) => refuse::<AnalysisReport>(Refusal::of("analysis_declined", message)),
    }
}

/// The sweep over a slot's defensible alternatives, for one quantity on one trial.
///
/// This is what answers "how much does the method choice move this number", so it takes no
/// option to enable it and sits beside the analysis rather than behind it.
pub fn spread_json(handle: &TrialHandle, request_json: &str) -> String {
    let request: SpreadRequest = match parse_request(request_json) {
        Ok(request) => request,
        Err(refusal) => return refuse::<SpreadResponse>(*refusal),
    };
    match spread::run(&handle.trial, &request) {
        Ok(response) => ok(response),
        Err(message) => refuse::<SpreadResponse>(Refusal::of("spread_declined", message)),
    }
}

/// Known doubles, written as JSON and declared beside their exact bit patterns.
///
/// A number reaching R crosses this boundary as text, and what R holds afterwards is
/// whatever the reader on this side made of those digits. The writer is correct; a parser
/// that is not correctly rounded loses the last bit on a few values in a hundred, and a
/// value that is one bit out is a value a manuscript reports wrongly. The probe declares
/// the bits so R can compare what it received against what was sent.
pub fn double_probe_json(count: usize) -> String {
    #[derive(Serialize)]
    struct Probe {
        values: Vec<f64>,
        bits: Vec<String>,
    }

    let mut state: u64 = 0x2026_0801_0000_0001;
    let mut values = Vec::with_capacity(count);
    let mut bits = Vec::with_capacity(count);
    for _ in 0..count {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        // Force-plate quantities span roughly 1e-3 to 1e4, so the mantissa is swept over
        // that range rather than over every representable double.
        let mantissa = (state >> 11) as f64 / (1u64 << 53) as f64;
        let exponent = ((state >> 3) % 8) as i32 - 3;
        let value = mantissa * 10f64.powi(exponent);
        values.push(value);
        bits.push(format!("{:016x}", value.to_bits()));
    }
    ok(Probe { values, bits })
}

/// One account per quantity, generated by the engine from the record the analysis
/// produced. Nothing on the R side assembles a sentence.
fn descriptions_of(
    response: &AnalysisResponse,
    registry_digest: &Option<String>,
) -> BTreeMap<String, String> {
    let bound: BTreeMap<&str, &plateforce_analysis::BoundMethod> = response
        .bound_methods
        .iter()
        .map(|method| (method.method_id.as_str(), method))
        .collect();

    let mut accounts = BTreeMap::new();
    for metric in &response.metrics {
        let Some(value) = metric.value else { continue };
        let inputs: Vec<ProvenanceChain> = metric
            .contributing_method_ids
            .iter()
            .filter_map(|id| bound.get(id.as_str()))
            .map(|method| {
                ProvenanceChain::leaf(provenance_of(method, registry_digest))
                    .choosing(method.enumerated_choices())
            })
            .collect();

        let own = Provenance {
            method_id: metric.computed_by.unwrap_or_default().to_string(),
            bound_parameters: Vec::new(),
            registry_version: None,
            registry_digest: registry_digest.clone(),
            // No acquisition block reaches this binding yet, and a dataset that cannot
            // fill one fingerprints as incomplete rather than as matching.
            acquisition_complete: false,
        };
        let measured = Measured {
            value,
            unit: metric.unit,
            provenance: own.clone(),
        };
        let chain = ProvenanceChain::with_inputs(own, inputs);
        accounts.insert(metric.key.to_string(), describe(&measured, &chain));
    }
    accounts
}

fn provenance_of(
    method: &plateforce_analysis::BoundMethod,
    registry_digest: &Option<String>,
) -> Provenance {
    Provenance {
        method_id: method.method_id.clone(),
        bound_parameters: method.quantities(),
        registry_version: None,
        registry_digest: registry_digest.clone(),
        acquisition_complete: false,
    }
}
