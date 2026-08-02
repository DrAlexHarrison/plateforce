//! Batch in the browser.
//!
//! The tab has no filesystem, so a run arrives as an array of named text rather than as a
//! directory, and the same engine serves both. Every number stays in the tab: this module
//! makes no outbound request and the file never leaves the machine, which is the reason the
//! browser build is weighted as heavily as it is.

use plateforce_batch::{analyse, BatchRequest, SourceFormat, TrialIdentity, TrialSet};
use serde::Deserialize;
use wasm_bindgen::prelude::*;

use crate::registry_embed;

/// One file a person dropped on the page.
#[derive(Deserialize)]
struct DroppedFile {
    name: String,
    text: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BrowserBatchRequest {
    files: Vec<DroppedFile>,
    format: SourceFormat,
    identity: TrialIdentity,
    analysis: plateforce_analysis::AnalysisRequest,
    /// Constructs the person resolved by an explicit act rather than by arriving at a default.
    #[serde(default)]
    resolved: Vec<String>,
}

/// Run a batch over dropped files and return the envelope every surface returns.
///
/// The return is the same string the library and Python produce for the same input, so the
/// three can be compared byte for byte rather than approximately.
#[wasm_bindgen(js_name = batchJson)]
pub fn batch_json(request_json: &str) -> Result<String, JsError> {
    let request: BrowserBatchRequest = serde_json::from_str(request_json)
        .map_err(|error| JsError::new(&format!("the batch request did not parse: {error}")))?;

    let sources: Vec<(String, String)> = request
        .files
        .into_iter()
        .map(|file| (file.name, file.text))
        .collect();
    let set = TrialSet::from_sources(sources, &request.format, &request.identity)
        .map_err(|error| JsError::new(&error.to_string()))?;

    let loaded = registry_embed::load()
        .map_err(|error| JsError::new(&format!("the embedded registry did not load: {error}")))?;

    let resolved: Vec<&str> = request.resolved.iter().map(String::as_str).collect();
    let batch = BatchRequest::new(request.analysis).resolving(&resolved);

    Ok(plateforce_batch::envelope(&analyse(
        &set,
        &batch,
        &loaded.registry,
    )))
}

/// What a run walked, so a page over ten seconds fills in counts rather than spinning.
#[wasm_bindgen(js_name = batchCoverage)]
pub fn batch_coverage(request_json: &str) -> Result<String, JsError> {
    let envelope = batch_json(request_json)?;
    let value: serde_json::Value = serde_json::from_str(&envelope)
        .map_err(|error| JsError::new(&error.to_string()))?;
    let run = value
        .get("ok")
        .and_then(|ok| ok.get("run"))
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    Ok(run.to_string())
}
