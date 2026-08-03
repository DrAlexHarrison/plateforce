//! WebAssembly surface.
//!
//! The browser build runs the same compiled logic as the desktop, so nothing in this
//! crate computes a quantity and nothing here decides a method. It reads files, holds a
//! trial, forwards to `plateforce_analysis`, and hands back JSON.
//!
//! The boundary is JSON strings rather than a structural bridge. That holds the
//! dependency tree at wasm-bindgen and serde, and no payload in this project is large
//! enough for the difference to be measurable: a trial is 6,000 samples.
//!
//! No threads. Threads would require `SharedArrayBuffer`, which would require
//! cross-origin isolation headers, which static hosting does not serve.

pub mod batch;
pub mod demo;
pub mod registry_embed;

use serde::Serialize;
use wasm_bindgen::prelude::*;

use plateforce_analysis::binding::SPINE_CONSTRUCTS;
use plateforce_analysis::capability::{capability, Operation, OutputFormat};
use plateforce_analysis::{spread, AnalysisRequest, Binding, BINDINGS};
use plateforce_core::read;
use plateforce_core::signal::{partition_sentinels, Sentinel};
use plateforce_core::Trial;

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[derive(Serialize)]
struct BuildInfo {
    version: &'static str,
    /// The revision the registry names itself, so a document written here cites the same
    /// name the terminal and the wheel do rather than a null.
    registry_version: Option<String>,
    registry_digest: String,
    registry_file_count: usize,
    registry_valid: bool,
    registry_violations: Vec<String>,
    /// Every rule that runs, with the slot it fills.
    bindings: &'static [Binding],
    /// The constructs the request names by its own fields. `Dispatch` carries a function
    /// pointer and is not serialised, and a binding's slot word equals its construct id for
    /// takeoff as well as for every derived row, so an interface reading `bindings` alone
    /// cannot tell which of the two ways a construct is asked for.
    spine_constructs: &'static [&'static str],
    threads: bool,
}

/// Everything the interface needs to describe this build, including which registry it
/// was compiled against and whether that registry passes its own validator. A number is
/// only reproducible alongside this.
#[wasm_bindgen(js_name = buildInfoJson)]
pub fn build_info_json() -> Result<String, JsError> {
    let loaded = registry_embed::load().map_err(|e| JsError::new(&e.to_string()))?;
    to_json(&BuildInfo {
        version: version(),
        registry_version: loaded.registry.declared_version.clone(),
        registry_digest: loaded.digest.clone(),
        registry_file_count: loaded.file_count,
        registry_valid: loaded.is_valid(),
        registry_violations: loaded.violation_messages(),
        bindings: BINDINGS,
        spine_constructs: SPINE_CONSTRUCTS,
        threads: false,
    })
}

#[derive(Serialize)]
struct Census {
    constructs: usize,
    computation_entries: usize,
    protocol_entries: usize,
    preset_entries: usize,
}

#[derive(Serialize)]
struct RegistryView<'a> {
    digest: String,
    valid: bool,
    violations: Vec<String>,
    constructs: Vec<&'a plateforce_registry::Construct>,
    methods: Vec<&'a plateforce_registry::Method>,
    protocols: Vec<&'a plateforce_registry::Protocol>,
    /// The published pipelines, in full. A tab that received only the count could report how
    /// many there are and offer none of them.
    presets: Vec<&'a plateforce_registry::Preset>,
    census: Census,
}

/// The registry as data, for a picker driven by the registry rather than by a hardcoded
/// list. Adding a method is a file edit and the interface follows.
#[wasm_bindgen(js_name = registryJson)]
pub fn registry_json() -> Result<String, JsError> {
    let loaded = registry_embed::load().map_err(|e| JsError::new(&e.to_string()))?;
    let census = loaded.registry.census();
    to_json(&RegistryView {
        digest: loaded.digest.clone(),
        valid: loaded.is_valid(),
        violations: loaded.violation_messages(),
        constructs: loaded.registry.constructs.values().collect(),
        methods: loaded.registry.methods.values().collect(),
        protocols: loaded.registry.protocols.values().collect(),
        presets: loaded.registry.presets.values().collect(),
        census: Census {
            constructs: census.constructs,
            computation_entries: census.computation_entries,
            protocol_entries: census.protocol_entries,
            preset_entries: census.preset_entries,
        },
    })
}

/// The request a published pipeline resolves to, or the record saying why it did not.
///
/// One of the two is present. A refusal travels as the record every other surface receives
/// rather than as a sentence, so a tab branches on the code the terminal exits on.
#[derive(Serialize)]
struct AdoptedPreset {
    #[serde(skip_serializing_if = "Option::is_none")]
    request: Option<AnalysisRequest>,
    #[serde(skip_serializing_if = "Option::is_none")]
    refusal: Option<plateforce_core::Refusal>,
}

/// The request a named published pipeline resolves to, laid onto the one the tab has built.
///
/// Resolved here rather than in JavaScript. A second implementation of which values a
/// pipeline supplied would be a second answer to the question this product exists to answer,
/// and the interface reads the returned request back into its own controls, so a reader sees
/// exactly what the pipeline bound before anything is computed.
#[wasm_bindgen(js_name = adoptPreset)]
pub fn adopt_preset(preset_id: &str, request_json: &str) -> Result<String, JsError> {
    let loaded = registry_embed::load().map_err(|e| JsError::new(&e.to_string()))?;
    let mut request: AnalysisRequest =
        serde_json::from_str(request_json).map_err(|e| JsError::new(&e.to_string()))?;

    let refused = match plateforce_analysis::request::preset_named(&loaded.registry, preset_id) {
        Err(refusal) => Some(*refusal),
        Ok(preset) => request.adopt(preset).err().map(|refusal| *refusal),
    };
    to_json(&AdoptedPreset {
        request: refused.is_none().then_some(request),
        refusal: refused,
    })
}

/// Every entry point this crate hands JavaScript, and what each can be asked to do.
///
/// Keyed by the name JavaScript calls, so an export that goes away takes its operations
/// with it. The tab reaches `plateforce_batch::analyse` and not `compare`, so `batch`
/// appears here and `compare` does not.
/// None for a name with no arm, which the test below refuses, so an export reaching the
/// list without reaching this table cannot pass as one that dispatches nothing.
fn operations_named(export: &str) -> Option<&'static [Operation]> {
    match export {
        "analyse" => Some(&[Operation::Analyse, Operation::Spread]),
        // Shaping the request a pipeline resolves to, which is part of analysing one trial
        // rather than a question of its own.
        "adoptPreset" => Some(&[Operation::Analyse]),
        "spread" => Some(&[Operation::Spread]),
        "parse" => Some(&[Operation::ParseForceFile]),
        "batchJson" | "batchCoverage" => Some(&[Operation::Batch]),
        "capabilityJson" => Some(&[Operation::Capability]),
        // One call answers all three: it returns the census, every entry in full, and
        // whether the registry it was compiled against passes its own validator.
        "registryJson" => Some(&[
            Operation::RegistryCensus,
            Operation::RegistryShow,
            Operation::RegistryValidate,
        ]),
        "buildInfoJson" => Some(&[Operation::Version, Operation::RegistryValidate]),
        // Loading a trial, drawing it, and describing what was loaded. None of the four is
        // a computation this manifest asserts over.
        "fromForceFile" | "demonstration" | "infoJson" | "envelopeJson" | "summaryJson" => {
            Some(&[])
        }
        _ => None,
    }
}

/// Every name this crate exposes to JavaScript. The test below reads the same names out of
/// the source, so an export added or removed without reaching this list fails there.
const EXPORTS: &[&str] = &[
    "adoptPreset",
    "analyse",
    "batchCoverage",
    "batchJson",
    "buildInfoJson",
    "capabilityJson",
    "demonstration",
    "envelopeJson",
    "fromForceFile",
    "infoJson",
    "parse",
    "registryJson",
    "spread",
    "summaryJson",
];

/// What this surface can be asked to do, reported by this surface.
///
/// The browser answers the same question the terminal and the wheel answer, in the same
/// shape. Nothing here reports a build digest: the same commit produces three different
/// wasm digests on three runners, so a digest would make this a comparison that can only
/// fail. Every field is semantic, ids and slots and constructs and refusal codes.
#[wasm_bindgen(js_name = capabilityJson)]
pub fn capability_json() -> Result<String, JsError> {
    let operations: Vec<Operation> = EXPORTS
        .iter()
        .filter_map(|export| operations_named(export))
        .flatten()
        .copied()
        .collect();
    replied(&capability(&operations, &[OutputFormat::Json]))
}

/// A parsed force file, before any column has been declared to be the force channel.
#[wasm_bindgen]
pub struct ForceFile {
    inner: read::ForceFile,
}

#[wasm_bindgen]
impl ForceFile {
    /// Parse text that came from a file input. The bytes never leave the tab.
    #[wasm_bindgen(js_name = parse)]
    pub fn parse_text(text: &str) -> Result<ForceFile, JsError> {
        read::parse(text)
            .map(|inner| ForceFile { inner })
            .map_err(|e| JsError::new(&e.to_string()))
    }

    /// What the reader found and what it decided, including the columns it did not pick.
    #[wasm_bindgen(js_name = summaryJson)]
    pub fn summary_json(&self) -> Result<String, JsError> {
        to_json(&self.inner.summary)
    }
}

#[derive(Serialize)]
struct LoadedTrialInfo {
    sample_count: usize,
    sample_rate_hz: f64,
    duration_seconds: f64,
    force_column: usize,
    sentinel_convention: String,
    sentinel_samples_replaced: usize,
    minimum_newtons: f64,
    maximum_newtons: f64,
    synthetic: bool,
}

/// One loaded trial. Held in WebAssembly memory for the life of the tab and written
/// nowhere.
#[wasm_bindgen]
pub struct LoadedTrial {
    trial: Trial,
    info: LoadedTrialInfo,
}

#[wasm_bindgen]
impl LoadedTrial {
    /// Bind a column of a parsed file to the vertical ground reaction force channel.
    ///
    /// The sentinel convention is stated by the caller and never inherited, because a
    /// silent choice here cannot be told apart from a measurement.
    #[wasm_bindgen(js_name = fromForceFile)]
    pub fn from_force_file(
        file: &ForceFile,
        force_column: usize,
        sample_rate_hz: f64,
        sentinel_convention: &str,
    ) -> Result<LoadedTrial, JsError> {
        let column = file.inner.columns.get(force_column).ok_or_else(|| {
            JsError::new(&format!(
                "column {} was requested but the file has {}",
                force_column + 1,
                file.inner.columns.len()
            ))
        })?;

        let sentinel = match sentinel_convention {
            "zero" => Some(Sentinel::Zero),
            "negative_one" => Some(Sentinel::NegativeOne),
            "none" => None,
            other => {
                return Err(JsError::new(&format!(
                    "unknown sentinel convention '{other}'"
                )))
            }
        };

        // A sentinel is not a measurement and neither is an unreadable field. Both are
        // held at the last real reading and counted, and the count is shown next to the
        // trace rather than folded into it.
        let flagged = sentinel.map(|s| partition_sentinels(column, s).1);
        let mut replaced = 0usize;
        let mut force = Vec::with_capacity(column.len());
        for (index, value) in column.iter().enumerate() {
            let missing = !value.is_finite()
                || flagged
                    .as_ref()
                    .is_some_and(|dropped| dropped.binary_search(&index).is_ok());
            if missing {
                replaced += 1;
                force.push(force.last().copied().unwrap_or(0.0));
            } else {
                force.push(*value);
            }
        }

        let trial = Trial::new(force, sample_rate_hz).map_err(|e| JsError::new(&e.to_string()))?;
        let info = describe(&trial, force_column, sentinel_convention, replaced, false);
        Ok(LoadedTrial { trial, info })
    }

    /// The trial the interface opens with, so the tool is explorable with no data at hand.
    ///
    /// Recorded rather than drawn. Sweeping the published rules for the start of the jump
    /// moves the height by 1.9 cm on this trace and by 0.04 mm on the drawn one, so the
    /// drawn one cannot show what choosing a method costs.
    #[wasm_bindgen(js_name = demonstration)]
    pub fn demonstration() -> LoadedTrial {
        let trial = demo::recorded_countermovement_jump();
        let info = describe(&trial, 0, "none", 0, false);
        LoadedTrial { trial, info }
    }

    #[wasm_bindgen(js_name = infoJson)]
    pub fn info_json(&self) -> Result<String, JsError> {
        to_json(&self.info)
    }

    /// Per-bucket minimum and maximum, so a spike survives being drawn at 900 px wide
    /// instead of being averaged away.
    #[wasm_bindgen(js_name = envelopeJson)]
    pub fn envelope_json(&self, buckets: usize) -> Result<String, JsError> {
        let force = self.trial.force();
        let buckets = buckets.clamp(1, force.len());
        let width = force.len() as f64 / buckets as f64;
        let mut lower = Vec::with_capacity(buckets);
        let mut upper = Vec::with_capacity(buckets);
        for bucket in 0..buckets {
            let start = (bucket as f64 * width) as usize;
            let end = (((bucket + 1) as f64 * width) as usize).min(force.len());
            let slice = &force[start..end.max(start + 1)];
            lower.push(slice.iter().copied().fold(f64::INFINITY, f64::min));
            upper.push(slice.iter().copied().fold(f64::NEG_INFINITY, f64::max));
        }
        to_json(&Envelope {
            sample_count: force.len(),
            sample_rate_hz: self.trial.sample_rate_hz(),
            lower,
            upper,
        })
    }

    /// One analysis. Every number in the response names the methods that produced it, and a
    /// rule that declined arrives as the record it built rather than as a thrown sentence.
    #[wasm_bindgen(js_name = analyse)]
    pub fn analyse(&self, request_json: &str) -> Result<String, JsError> {
        let request: AnalysisRequest =
            serde_json::from_str(request_json).map_err(|e| JsError::new(&e.to_string()))?;
        match plateforce_analysis::run(&self.trial, &request) {
            Ok(response) => replied(&response),
            Err(refusal) => refused(&refusal),
        }
    }

    /// Every defensible alternative for one quantity, and how far the number moves.
    #[wasm_bindgen(js_name = spread)]
    pub fn spread(&self, request_json: &str) -> Result<String, JsError> {
        let request: spread::SpreadRequest =
            serde_json::from_str(request_json).map_err(|e| JsError::new(&e.to_string()))?;
        match spread::run(&self.trial, &request) {
            Ok(response) => replied(&response),
            Err(refusal) => refused(&refusal),
        }
    }
}

fn describe(
    trial: &Trial,
    force_column: usize,
    sentinel_convention: &str,
    sentinel_samples_replaced: usize,
    synthetic: bool,
) -> LoadedTrialInfo {
    LoadedTrialInfo {
        sample_count: trial.len(),
        sample_rate_hz: trial.sample_rate_hz(),
        duration_seconds: trial.duration_seconds(),
        force_column,
        sentinel_convention: sentinel_convention.to_string(),
        sentinel_samples_replaced,
        minimum_newtons: trial.force().iter().copied().fold(f64::INFINITY, f64::min),
        maximum_newtons: trial
            .force()
            .iter()
            .copied()
            .fold(f64::NEG_INFINITY, f64::max),
        synthetic,
    }
}

#[derive(Serialize)]
struct Envelope {
    sample_count: usize,
    sample_rate_hz: f64,
    lower: Vec<f64>,
    upper: Vec<f64>,
}

fn to_json<T: Serialize>(value: &T) -> Result<String, JsError> {
    serde_json::to_string(value).map_err(|e| JsError::new(&e.to_string()))
}

/// The envelope every surface returns, `{"ok": ...}` or `{"refusal": {...}}`.
///
/// A reply that can carry a refusal record is written this way, because a thrown string
/// loses every field a caller branches on: the code, the rule, the parameter and what could
/// have been asked for instead. `batchJson` already answered in this shape, and the terminal
/// and the R package answer in it too. The exports that stay on the throwing path are the
/// ones whose only failure is this bundle being broken, where there is nothing to branch to.
fn replied<T: Serialize>(value: &T) -> Result<String, JsError> {
    to_json(&serde_json::json!({ "ok": value }))
}

fn refused(refusal: &plateforce_core::Refusal) -> Result<String, JsError> {
    to_json(&serde_json::json!({ "refusal": refusal }))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every `js_name` in this crate's source, so the list the manifest is built from is
    /// checked against what the crate actually exposes rather than trusted.
    fn exports_in_the_source() -> Vec<String> {
        let marker = concat!("js_name", " = ");
        let mut found: Vec<String> = [include_str!("lib.rs"), include_str!("batch.rs")]
            .iter()
            .flat_map(|source| source.split(marker).skip(1))
            .filter_map(|tail| tail.split(')').next())
            .map(|name| name.trim().to_string())
            .filter(|name| {
                !name.is_empty()
                    && name
                        .chars()
                        .all(|character| character.is_ascii_alphanumeric() || character == '_')
            })
            .collect();
        found.sort();
        found.dedup();
        found
    }

    #[test]
    fn the_manifest_is_built_from_the_entry_points_this_crate_exposes() {
        let mut declared: Vec<String> = EXPORTS.iter().map(|name| name.to_string()).collect();
        declared.sort();
        let found = exports_in_the_source();
        assert!(
            found.len() >= 12,
            "the source scan found {} names, so it is matching nothing",
            found.len()
        );
        assert_eq!(declared, found);
    }

    /// An export reaching the list with no arm in the table would contribute nothing and
    /// read exactly like one that dispatches nothing on purpose.
    #[test]
    fn every_entry_point_states_what_it_dispatches() {
        let unstated: Vec<&&str> = EXPORTS
            .iter()
            .filter(|export| operations_named(export).is_none())
            .collect();
        assert!(unstated.is_empty(), "{unstated:?}");
    }

    /// A surface claiming what it cannot do is the one failure this manifest exists to make
    /// visible, so the two the tab does not reach are named rather than left to the diff.
    #[test]
    fn the_browser_claims_neither_compare_nor_reach() {
        let manifest = capability_json().expect("the manifest serialises");
        assert!(!manifest.contains("\"compare\""), "{manifest}");
        assert!(!manifest.contains("\"reach\""), "{manifest}");
        assert!(manifest.contains("\"batch\""), "{manifest}");
    }

    /// JSON strings are the whole boundary of this crate, so a container format arriving
    /// here without a writer behind it would be a claim about nobody's code.
    #[test]
    fn the_only_container_the_tab_writes_is_json() {
        let manifest = capability_json().expect("the manifest serialises");
        assert!(
            manifest.contains("\"output_formats\":[\"json\"]"),
            "{manifest}"
        );
    }
}
