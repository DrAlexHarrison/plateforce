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

pub mod demo;
pub mod parse;
pub mod registry_embed;

use serde::Serialize;
use wasm_bindgen::prelude::*;

use plateforce_analysis::{spread, AnalysisRequest, Binding, BINDINGS};
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
        threads: false,
    })
}

#[derive(Serialize)]
struct Census {
    constructs: usize,
    computation_entries: usize,
    protocol_entries: usize,
}

#[derive(Serialize)]
struct RegistryView<'a> {
    digest: String,
    valid: bool,
    violations: Vec<String>,
    constructs: Vec<&'a plateforce_registry::Construct>,
    methods: Vec<&'a plateforce_registry::Method>,
    protocols: Vec<&'a plateforce_registry::Protocol>,
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
        census: Census {
            constructs: census.constructs,
            computation_entries: census.computation_entries,
            protocol_entries: census.protocol_entries,
        },
    })
}

/// A parsed force file, before any column has been declared to be the force channel.
#[wasm_bindgen]
pub struct ForceFile {
    inner: parse::ForceFile,
}

#[wasm_bindgen]
impl ForceFile {
    /// Parse text that came from a file input. The bytes never leave the tab.
    #[wasm_bindgen(js_name = parse)]
    pub fn parse_text(text: &str) -> Result<ForceFile, JsError> {
        parse::parse(text)
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

    /// The synthetic trial the interface opens with, so the tool is explorable with no
    /// data at hand and no recorded subject involved.
    #[wasm_bindgen(js_name = demonstration)]
    pub fn demonstration() -> LoadedTrial {
        let trial = demo::synthetic_countermovement_jump();
        let info = describe(&trial, 0, "none", 0, true);
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

    /// One analysis. Every number in the response names the methods that produced it.
    #[wasm_bindgen(js_name = analyse)]
    pub fn analyse(&self, request_json: &str) -> Result<String, JsError> {
        let request: AnalysisRequest =
            serde_json::from_str(request_json).map_err(|e| JsError::new(&e.to_string()))?;
        let response =
            plateforce_analysis::run(&self.trial, &request).map_err(|e| JsError::new(&e))?;
        to_json(&response)
    }

    /// Every defensible alternative for one quantity, and how far the number moves.
    #[wasm_bindgen(js_name = spread)]
    pub fn spread(&self, request_json: &str) -> Result<String, JsError> {
        let request: spread::SpreadRequest =
            serde_json::from_str(request_json).map_err(|e| JsError::new(&e.to_string()))?;
        let response = spread::run(&self.trial, &request).map_err(|e| JsError::new(&e))?;
        to_json(&response)
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
