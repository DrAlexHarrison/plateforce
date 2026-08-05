//! Batch in the browser.
//!
//! The tab has no filesystem, so a run arrives as an array of named text rather than as a
//! directory, and the same engine serves both. Every number stays in the tab: this module
//! makes no outbound request and the file never leaves the machine, which is the reason the
//! browser build is weighted as heavily as it is.

use plateforce_batch::{
    analyse, with_aggregates, AggregationRequest, BatchRequest, GroupKind, SourceFormat,
    TrialIdentity, TrialSet,
};
use plateforce_core::DispersionEstimator;
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
    /// What the person said about the plate every file in the folder came off. Stated once
    /// for the run, because a trace of forces carries none of it.
    #[serde(default)]
    capture: crate::StatedCapture,
    /// The published rule that reduces an athlete's trials to one number, and the count it was
    /// asked for. Absent leaves the run unreduced, which is what this surface did on every run
    /// it could make.
    ///
    /// `trial.aggregation` publishes three rules and none of them is the arithmetic mean of a
    /// session, so a tab that reduced without being told which would be taking a mean nobody
    /// chose, which is the defect this whole product exists to prevent.
    #[serde(default)]
    aggregate: Option<BrowserAggregation>,
}

/// What the tab was told to reduce, and how.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BrowserAggregation {
    /// The registry rule, by name. An unpublished name is refused rather than approximated.
    rule: String,
    /// The trial count the rule was asked for. It travels with the value everywhere it is
    /// reported, because best of five and best of three are different numbers.
    n: usize,
    /// subject, session or run.
    #[serde(default = "subject_grouping")]
    by: String,
    /// The quantities to reduce, or every quantity the run computed where none is named.
    #[serde(default)]
    quantities: Vec<String>,
    #[serde(default = "sample_dispersion")]
    dispersion: String,
}

fn subject_grouping() -> String {
    "subject".to_string()
}

fn sample_dispersion() -> String {
    "sample".to_string()
}

/// Run a batch over dropped files and return the envelope every surface returns.
///
/// The return is the same string the library and Python produce for the same input, so the
/// three can be compared byte for byte rather than approximately.
#[wasm_bindgen(js_name = batchJson)]
pub fn batch_json(request_json: &str) -> Result<String, JsError> {
    batch_document(request_json).map_err(|message| JsError::new(&message))
}

/// The same run, answering with the sentence rather than the exception.
///
/// `JsError` cannot be constructed on a non-wasm build and panics if a native caller reaches
/// one, so a test of this path could only ever run in a browser. `scripts/check-batch.mjs`
/// still drives the real page, which is the only thing that proves a route from the drop zone
/// to here; this is what lets the run itself be asked a question without one.
pub fn batch_document(request_json: &str) -> Result<String, String> {
    let request: BrowserBatchRequest = serde_json::from_str(request_json)
        .map_err(|error| format!("the batch request did not parse: {error}"))?;

    let sources: Vec<(String, String)> = request
        .files
        .into_iter()
        .map(|file| (file.name, file.text))
        .collect();
    let set = TrialSet::from_sources(sources, &request.format, &request.identity)
        .map_err(|error| error.to_string())?;

    let loaded = registry_embed::load()
        .map_err(|error| format!("the embedded registry did not load: {error}"))?;

    let resolved: Vec<&str> = request.resolved.iter().map(String::as_str).collect();
    let batch = BatchRequest::new(request.analysis)
        .resolving(&resolved)
        .describing(request.capture.resolved()?);

    let outcome = analyse(&set, &batch, &loaded.registry);

    let Some(asked) = request.aggregate else {
        return Ok(plateforce_batch::envelope(&outcome));
    };

    // A run that refused produced no rows, so there is nothing to reduce, and its refusal is
    // the answer. It travels in the envelope rather than as an exception, which is how this
    // surface returns a run refusal whether or not a reduction was asked for.
    let result = match outcome {
        Ok(result) => result,
        refused => return Ok(plateforce_batch::envelope(&refused)),
    };

    let group_kind = match asked.by.as_str() {
        "subject" => GroupKind::Subject,
        "session" => GroupKind::Session,
        "run" => GroupKind::Run,
        other => {
            return Err(format!(
                "a reduction is taken over subject, session or run, and this one named {other}"
            ))
        }
    };
    let dispersion =
        DispersionEstimator::from_published_str(&asked.dispersion).ok_or_else(|| {
            format!(
                "the standard deviation beside a reduced value is one of {}, and this run named {}",
                DispersionEstimator::PUBLISHED.join(", "),
                asked.dispersion,
            )
        })?;

    // Every quantity the run computed, where the tab named none. A scope rather than a method
    // choice, and each row names the quantity it reduced, so nothing is reduced unseen.
    let quantities = if asked.quantities.is_empty() {
        result.quantities.clone()
    } else {
        let absent: Vec<&str> = asked
            .quantities
            .iter()
            .filter(|key| !result.quantities.contains(key))
            .map(String::as_str)
            .collect();
        if !absent.is_empty() {
            return Err(format!(
                "this run computed {}, and a reduction was asked for {}",
                result.quantities.join(", "),
                absent.join(", "),
            ));
        }
        asked.quantities.clone()
    };

    let reduction = AggregationRequest::declared(
        Some(asked.rule.as_str()),
        Some(asked.n),
        group_kind,
        quantities,
        dispersion,
    )
    .map_err(|refusal| refusal.message().to_string())?;

    let reduced = with_aggregates(result, &set, &reduction)
        .map_err(|refusal| refusal.message().to_string())?;
    Ok(plateforce_batch::envelope(&Ok(reduced)))
}

/// What a run walked, so a page over ten seconds fills in counts rather than spinning.
#[wasm_bindgen(js_name = batchCoverage)]
pub fn batch_coverage(request_json: &str) -> Result<String, JsError> {
    let envelope = batch_json(request_json)?;
    let value: serde_json::Value =
        serde_json::from_str(&envelope).map_err(|error| JsError::new(&error.to_string()))?;
    let run = value
        .get("ok")
        .and_then(|ok| ok.get("run"))
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    Ok(run.to_string())
}
