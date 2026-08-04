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

use plateforce_analysis::capability::{capability, Operation, OutputFormat};
use plateforce_analysis::document::SpreadDocument;
use plateforce_analysis::spread::{self, SpreadRequest};
use plateforce_analysis::{run, AnalysisRequest, AnalysisResponse, Binding, BINDINGS};
use plateforce_core::acquisition::Acquisition;
use plateforce_core::read::read_delimited_column;
use plateforce_core::reporting::describe;
use plateforce_core::signal::reported_samples;
use plateforce_core::{Measured, Provenance, ProvenanceChain, Sentinel, Trial};
use plateforce_registry::{Method, Registry};
use serde::{Deserialize, Serialize};

/// The fields an R condition carries, named as the cross-surface idiom names them, so the
/// R class vector is `paste0("plateforce_", code)` with no translation table in between.
///
/// Field for field the same shape as `plateforce_core::Refusal`, which is what a rule's own
/// refusal arrives as on an analysis result. Two shapes would mean `cnd[["detail"]]` reading
/// as a named list out of one path and as a sentence out of the other, in one package.
#[derive(Debug, Clone, Serialize)]
pub struct Refusal {
    pub code: String,
    pub message: String,
    pub method_id: Option<String>,
    pub slot: Option<String>,
    pub parameter: Option<String>,
    pub value: Option<f64>,
    /// The declined value where the parameter's values are names rather than numbers.
    pub named_value: Option<String>,
    pub detail: std::collections::BTreeMap<String, f64>,
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
            named_value: None,
            detail: std::collections::BTreeMap::new(),
            available: None,
        }
    }

    fn naming_parameter(code: &str, parameter: &str, message: String) -> Self {
        Self {
            parameter: Some(parameter.to_string()),
            ..Self::of(code, message)
        }
    }

    /// The value a rule declined on, when it is a name rather than a number.
    fn about(mut self, value: impl Into<String>) -> Self {
        self.named_value = Some(value.into());
        self
    }
}

/// A refusal the engine built, in the shape this package hands to R.
///
/// Nothing is decided here: the code, the sentence and every field come from the record the
/// engine produced. Written as a conversion so a site that has one stops inventing a code
/// for the failure it already describes.
impl From<plateforce_core::Refusal> for Refusal {
    fn from(refusal: plateforce_core::Refusal) -> Self {
        Self {
            code: refusal.code.wire_name().to_string(),
            message: refusal.message().to_string(),
            method_id: Some(refusal.method_id.clone()).filter(|id| !id.is_empty()),
            slot: refusal.slot.clone(),
            parameter: refusal.parameter.clone(),
            value: refusal.value,
            named_value: refusal.named_value.clone(),
            detail: refusal.detail.clone(),
            available: Some(refusal.available.clone()).filter(|names| !names.is_empty()),
        }
    }
}

/// Boxed, because a refusal carrying every field a caller branches on is wide against an
/// answer of nothing, and every reply in this package is one of these two.
#[derive(Serialize)]
#[serde(untagged)]
enum Envelope<T: Serialize> {
    Ok { ok: T },
    Refused { refusal: Box<Refusal> },
}

fn ok<T: Serialize>(value: T) -> String {
    encode(Envelope::Ok { ok: value })
}

/// Takes either shape, because half this package's refusal sites already hold a box.
fn refuse<T: Serialize>(refusal: impl Into<Box<Refusal>>) -> String {
    encode(Envelope::<T>::Refused {
        refusal: refusal.into(),
    })
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
    /// The revision this registry names about itself, from the `VERSION` file beside its
    /// rules, and None where it names none. Outside the digest, which is measured over the
    /// `toml` files alone, so an R session could not recover it from anything else it holds.
    declared_version: Option<String>,
    census: Vec<CensusRow>,
    method_ids: Vec<String>,
    construct_ids: Vec<String>,
    protocol_ids: Vec<String>,
    /// The published pipelines this registry carries, so an R session can name one rather
    /// than only count them.
    preset_ids: Vec<String>,
}

/// The census, one row per population, each derived count beside the denominator it was
/// taken over. No total row and no total column: the two populations are not one
/// population, and a table carrying a total invites adding them.
pub fn registry_json(root: &str) -> String {
    let registry = match Registry::load(root) {
        Ok(registry) => registry,
        Err(error) => {
            return refuse::<RegistryReport>(Refusal::of("registry_invalid", error.to_string()))
        }
    };
    let census = registry.census();
    ok(RegistryReport {
        root: root.to_string(),
        digest: registry.content_digest.clone(),
        declared_version: registry.declared_version.clone(),
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
            CensusRow {
                population: "preset_entries".to_string(),
                count: census.preset_entries,
                genuine_debates: None,
                can_find_wrong_event: None,
            },
        ],
        method_ids: registry.methods.keys().cloned().collect(),
        construct_ids: registry.constructs.keys().cloned().collect(),
        protocol_ids: registry.protocols.keys().cloned().collect(),
        preset_ids: registry.presets.keys().cloned().collect(),
    })
}

/// One entry, serialised through the registry's own schema types, so no field is dropped
/// on the way to R and no second spelling of a field name exists here.
pub fn registry_entry_json(root: &str, id: &str) -> String {
    let registry = match Registry::load(root) {
        Ok(registry) => registry,
        Err(error) => return refuse::<Method>(Refusal::of("registry_invalid", error.to_string())),
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

/// The members of the acquisition block, named by the block itself, so a member added
/// upstream is one an R caller can be held to rather than one R silently never asks for.
pub fn acquisition_members_json() -> String {
    ok(Acquisition::MEMBERS)
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
    acquisition: Acquisition,
}

/// What the reader did, rather than what it assumed. Both silent exclusion and silent
/// inclusion are reportable here, and a reader is where both happen.
#[derive(Serialize, Clone)]
pub struct TrialReport {
    sample_count: usize,
    sample_rate_hz: f64,
    duration_seconds: f64,
    sentinel_convention: String,
    /// The two reasons a sample is reported, apart. One number carried both, and under the
    /// zero convention on a recording with three unreadable samples it read 160, of which
    /// 157 are an athlete in the air and 3 are the gap. This reader was the only one that
    /// counted the gap at all when no convention was declared, and even then it could report
    /// one of the two facts and not both.
    samples_matching_the_convention: usize,
    samples_carrying_no_number: usize,
    source: String,
    delimiter: Option<String>,
    force_column: Option<usize>,
    rows_read: Option<usize>,
    columns_per_row: Option<usize>,
    blank_lines_skipped: Option<usize>,
    /// False when any member of the acquisition block is absent, in which case results from
    /// this trial can never be declared to match another lab's.
    acquisition_complete: bool,
    /// The members still to find, named by the block itself.
    acquisition_missing: Vec<&'static str>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TrialRequest {
    sample_rate_hz: Option<f64>,
    #[serde(default)]
    sentinel_convention: Option<String>,
    /// Deserialised into the block core declares, so a member added there arrives here
    /// without an edit.
    #[serde(default)]
    acquisition: Acquisition,
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
    #[serde(default)]
    acquisition: Acquisition,
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
                "sentinel_convention_unknown",
                "sentinel_convention",
                format!("{other} is not a sentinel convention this reader applies"),
            )
            .about(other)
        })),
    }
}

/// A sample matching the declared convention is counted and left where it is.
///
/// Closing the gap would shift every timestamp after it, and holding the sample at the last
/// real reading writes a force the plate never measured into the trace. The second is the
/// worse of the two, because a zero sentinel is physically indistinguishable from a correct
/// reading during flight: an unloaded plate reads zero or one quantisation step, and a vendor
/// writing `0.00` to mean "no measurement" writes the same bytes. On the recorded trial 157
/// samples between indices 5029 and 5719 are exactly zero and all of them are inside the
/// flight phase, so holding carries a standing force across a flight that has none, and
/// takeoff and touchdown are both placed by a force threshold. Measured on that trial with
/// three quiet-stance samples zeroed, holding moved jump height 2.06 cm and time to takeoff
/// 69 ms away from what the terminal and Python report from the same file.
///
/// The counting itself is `plateforce_core::signal::reported_samples`, which is where the
/// policy lives for every surface. This reader used to spell it here with a
/// `Sentinel::Value(f64::NAN)` standing in for no convention at all, which counted the gaps
/// and could never separate them from the convention's own matches.
fn build_trial(
    values: &[f64],
    sample_rate_hz: Option<f64>,
    convention: Option<String>,
    source: String,
    acquisition: Acquisition,
) -> Result<TrialHandle, Box<Refusal>> {
    let sample_rate_hz = sample_rate_hz.ok_or_else(|| {
        Box::new(Refusal::naming_parameter(
            "required_parameter_unstated",
            "sample_rate_hz",
            "this trace carries no sample rate, so state the rate it was recorded at".to_string(),
        ))
    })?;
    let convention = convention.unwrap_or_else(|| "none".to_string());
    let sentinel = sentinel_from(&convention)?;
    let reported = reported_samples(values, sentinel);
    let trial = Trial::new(values.to_vec(), sample_rate_hz)
        .map_err(|error| Box::new(Refusal::of("trace_too_short", error.to_string())))?;
    let report = TrialReport {
        sample_count: trial.len(),
        sample_rate_hz: trial.sample_rate_hz(),
        duration_seconds: trial.duration_seconds(),
        sentinel_convention: convention,
        samples_matching_the_convention: reported.matched_the_convention,
        samples_carrying_no_number: reported.carried_no_number,
        source,
        delimiter: None,
        force_column: None,
        rows_read: None,
        columns_per_row: None,
        blank_lines_skipped: None,
        acquisition_complete: acquisition.is_complete(),
        acquisition_missing: acquisition.missing(),
    };
    Ok(TrialHandle {
        trial,
        report,
        acquisition,
    })
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
        request.acquisition,
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
                    "required_parameter_unstated",
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
                refuse::<TrialReport>(Refusal::from(plateforce_core::Refusal::file_not_read(
                    request.path.clone(),
                    error.to_string(),
                ))),
                None,
            )
        }
    };

    let (values, column) = match read_delimited_column(&text, delimiter, force_column) {
        Ok(read) => read,
        Err(error) => {
            return (
                refuse::<TrialReport>(Refusal::from(plateforce_core::Refusal::from(error))),
                None,
            )
        }
    };

    match build_trial(
        &values,
        request.sample_rate_hz,
        request.sentinel_convention,
        request.path.clone(),
        request.acquisition,
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
            "required_parameter_unstated",
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
    /// The revision the caller pinned, and null when they pinned none. Written as null
    /// rather than left out: a key this document sometimes omits cannot be told apart from a
    /// surface that never carried it, which is what R's document did until now.
    registry_version: Option<String>,
    /// The revision the registry names about itself, and null where it names none.
    registry_declared_version: Option<String>,
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
    /// The revision the caller pinned, absent when they pinned none. The R side is where the
    /// pin is written, because it is the caller's word and no compiled default could supply
    /// one without inventing a citation.
    #[serde(default)]
    registry_version: Option<String>,
    /// The revision the registry declares about itself, read on the R side from the same
    /// registry the digest was measured over, for the same reason it is.
    #[serde(default)]
    registry_declared_version: Option<String>,
}

impl AnalyseRequest {
    /// What this run says about the registry behind it. Built once so the report and the
    /// per-rule records cannot answer differently.
    fn stamp(&self) -> plateforce_core::provenance::RegistryStamp {
        plateforce_core::provenance::RegistryStamp {
            version: self.registry_version.clone(),
            declared_version: self.registry_declared_version.clone(),
            digest: self.registry_digest.clone(),
        }
    }
}

/// One analysis under a named published pipeline.
///
/// The pipeline is laid onto the request here, on the compiled side, because R writes
/// requests and does not read them: handing the resolved request back would make R the
/// second reader of a document this side already holds parsed. The record that comes back
/// names the pipeline against each rule it bound.
pub fn analyse_under_preset_json(
    handle: &TrialHandle,
    root: &str,
    preset_id: &str,
    request_json: &str,
) -> String {
    let registry = match Registry::load(root) {
        Ok(registry) => registry,
        Err(error) => {
            return refuse::<AnalysisReport>(Refusal::of("registry_invalid", error.to_string()))
        }
    };
    let mut request: AnalyseRequest = match parse_request(request_json) {
        Ok(request) => request,
        Err(refusal) => return refuse::<AnalysisReport>(*refusal),
    };
    let refused = match plateforce_analysis::request::preset_named(&registry, preset_id) {
        Err(refusal) => Some(refusal),
        Ok(preset) => request.analysis.adopt(preset).err(),
    };
    if let Some(refusal) = refused {
        return refuse::<AnalysisReport>(Refusal::from(*refusal));
    }
    run_and_report(handle, request)
}

pub fn analyse_json(handle: &TrialHandle, request_json: &str) -> String {
    let request: AnalyseRequest = match parse_request(request_json) {
        Ok(request) => request,
        Err(refusal) => return refuse::<AnalysisReport>(*refusal),
    };
    run_and_report(handle, request)
}

/// One home for running a request and shaping the report, so a run under a pipeline and a
/// run under rules the caller named produce the same document.
fn run_and_report(handle: &TrialHandle, request: AnalyseRequest) -> String {
    let complete = handle.acquisition.is_complete();
    let stamp = request.stamp();
    match run(&handle.trial, &request.analysis) {
        Ok(response) => ok(AnalysisReport {
            descriptions: descriptions_of(&response, &stamp, complete),
            response,
            registry_digest: stamp.digest.clone(),
            registry_version: stamp.version.clone(),
            registry_declared_version: stamp.declared_version.clone(),
            acquisition_complete: complete,
        }),
        // The code the engine decided it was declining under. This site used to wrap the
        // sentence under `analysis_declined`, which this package's own manifest does not
        // publish, so no caller could catch it by the class the manifest names.
        Err(declined) => refuse::<AnalysisReport>(Refusal::from(*declined)),
    }
}

/// The sweep over a slot's defensible alternatives, for one quantity on one trial.
///
/// This is what answers "how much does the method choice move this number", so it takes no
/// option to enable it and sits beside the analysis rather than behind it.
pub fn spread_json(handle: &TrialHandle, request_json: &str) -> String {
    let request: SweepRequest = match parse_request(request_json) {
        Ok(request) => request,
        Err(refusal) => return refuse::<SpreadDocument>(*refusal),
    };
    match spread::run(&handle.trial, &request.sweep) {
        Ok(response) => ok(SpreadDocument::of(
            env!("CARGO_PKG_VERSION"),
            &request.stamp(),
            response,
        )),
        Err(declined) => refuse::<SpreadDocument>(Refusal::from(*declined)),
    }
}

/// A sweep, and the registry identity the caller measured from the registry it loaded.
///
/// The shape `AnalyseRequest` already uses, and for the same reason: `SpreadRequest` refuses
/// unknown fields, so the identity cannot ride on the sweep itself, and reading the registry
/// again here would name one this call never loaded.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SweepRequest {
    #[serde(flatten)]
    sweep: SpreadRequest,
    registry_digest: Option<String>,
    /// The revision the caller pinned, absent where nobody pinned one. `docs/schema.md`
    /// gives this field that meaning and no other.
    registry_version: Option<String>,
    /// The revision the registry declares about itself, read on the R side from the same
    /// registry the digest was measured over, exactly as `AnalyseRequest` reads it.
    #[serde(default)]
    registry_declared_version: Option<String>,
}

impl SweepRequest {
    /// What this sweep says about the registry behind it, built the one way `AnalyseRequest`
    /// builds it so a sweep and an analysis of the same trial cannot answer differently.
    fn stamp(&self) -> plateforce_core::provenance::RegistryStamp {
        plateforce_core::provenance::RegistryStamp {
            version: self.registry_version.clone(),
            declared_version: self.registry_declared_version.clone(),
            digest: self.registry_digest.clone(),
        }
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
///
/// A quantity with no value gets no account, because an account is written around a
/// `Measured` and there is nothing measured to write one about. This used to be the one place
/// in the whole product where a number that is not a number could be told from a number
/// nobody computed: a metric could hold a NaN, so it reached this loop, and the account read
/// "NaN newtons" with a full provenance chain behind it, asserting a measurement that was
/// never made. That distinction now lives on the metric itself, on every surface, as
/// `carried_no_number`, so this loop is free to say nothing about a quantity that has no
/// value without taking the only account of it away from a reader.
fn descriptions_of(
    response: &AnalysisResponse,
    registry: &plateforce_core::provenance::RegistryStamp,
    acquisition_complete: bool,
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
                let record = method.into_provenance(registry, acquisition_complete, Vec::new());
                ProvenanceChain::leaf(record).choosing(method.enumerated_choices())
            })
            .collect();

        let own = Provenance {
            registry_version: registry.version.clone(),
            registry_declared_version: registry.declared_version.clone(),
            registry_digest: registry.digest.clone(),
            acquisition_complete,
            ..Provenance::of(metric.computed_by.clone().unwrap_or_default())
        };
        let Some(unit) = declared_unit(metric) else {
            continue;
        };
        let measured = Measured {
            value,
            unit,
            provenance: own.clone(),
        };
        let chain = ProvenanceChain::with_inputs(own, inputs);
        accounts.insert(metric.key.to_string(), describe(&measured, &chain));
    }
    accounts
}

/// The unit a metric reports, taken from the one declaration that spells it, and only when
/// the metric agrees with that declaration.
///
/// A metric owns its unit so a quantity can take one from the registry, and `Measured` holds
/// a static one. Reading the declaration instead of the metric would print a unit the number
/// does not carry the moment those two differ, which `unit_of_every_metric_is_the_declared_one`
/// is what stops.
fn declared_unit(metric: &plateforce_analysis::Metric) -> Option<&'static str> {
    let declared = plateforce_analysis::response::quantity(&metric.key)?;
    (declared.unit == metric.unit).then_some(declared.unit)
}

/// What this surface can be asked to do, reported by naming the entry points it dispatches
/// rather than by forwarding a document every surface would agree with.
///
/// `output_formats` is empty because this surface writes no file. A surface that claimed a
/// container it cannot write would pass a comparison and fail a user.
pub fn capability_json() -> String {
    let operations = [
        Operation::Analyse,
        Operation::Capability,
        Operation::ParseForceFile,
        Operation::RegistryCensus,
        Operation::RegistryShow,
        Operation::Spread,
        Operation::Version,
    ];
    let formats: [OutputFormat; 0] = [];
    match serde_json::to_value(capability(&operations, &formats)) {
        Ok(value) => canonical(&value),
        Err(error) => {
            refuse::<serde_json::Value>(Refusal::of("serialisation_failed", error.to_string()))
        }
    }
}

/// Sorted keys and no spacing, so a comparison against another surface is a plain diff and
/// not a question about which map type a build selected.
fn canonical(value: &serde_json::Value) -> String {
    serde_json::to_string(&sorted(&serde_json::json!({ "ok": value })))
        .unwrap_or_else(|_| String::from("{}"))
}

fn sorted(value: &serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            let mut out = serde_json::Map::new();
            for key in keys {
                out.insert(key.clone(), sorted(&map[key]));
            }
            serde_json::Value::Object(out)
        }
        serde_json::Value::Array(items) => {
            serde_json::Value::Array(items.iter().map(sorted).collect())
        }
        other => other.clone(),
    }
}
