//! A request, written out and read back, is the same request.
//!
//! The document exists so a colleague reproduces an analysis without a conversation, which
//! is a claim about a round trip rather than about a shape. It is tested here rather than
//! beside the type because `plateforce-analysis` carries no JSON serialiser, and adding one
//! to a single-owner manifest to avoid moving a test file would be the wrong trade.

use std::collections::BTreeMap;

use plateforce_analysis::method_set::{MethodSet, MethodSetBinding, METHOD_SET_SCHEMA};
use plateforce_analysis::{AnalysisRequest, MethodChoice, WeighingChoice};
use plateforce_core::STANDARD_GRAVITY_METERS_PER_SECOND_SQUARED;

fn owen_request() -> AnalysisRequest {
    AnalysisRequest {
        weighing: WeighingChoice {
            method_id: "bwepoch.fixed_window".into(),
            start_index: None,
            parameters: BTreeMap::from([("duration".to_string(), 1.0)]),
            options: BTreeMap::new(),
            ..Default::default()
        },
        onset: MethodChoice {
            method_id: "onset.threshold.noise_relative".into(),
            parameters: BTreeMap::from([("k".to_string(), 5.0)]),
            ..Default::default()
        },
        takeoff: MethodChoice {
            method_id: "takeoff.threshold.absolute_force".into(),
            parameters: BTreeMap::from([("threshold_n".to_string(), 20.0)]),
            ..Default::default()
        },
        touchdown_index: None,
        gravity_meters_per_second_squared: STANDARD_GRAVITY_METERS_PER_SECOND_SQUARED,
        registry_backed_ids: Vec::new(),
        ..Default::default()
    }
}

fn document_of(request: &AnalysisRequest) -> MethodSet {
    // A registry that declares one revision, read by a caller who pinned another. The two
    // are deliberately different: a document that used one string for both would read as
    // correct whichever field the writer put it in.
    MethodSet::of(
        request,
        "0.1.0",
        &plateforce_core::provenance::RegistryStamp::unpinned(
            Some("2026-07-25".into()),
            Some("fnv1a-deadbeef".into()),
        )
        .pinned_to(Some("my-lab-2026-03".into())),
    )
}

/// Every binding of the request, as a comparable list.
///
/// The round trip is anchored to the request rather than to a second document, because
/// comparing two documents compares two outputs of the same function: a writer that drops a
/// parameter drops it from both sides and the comparison passes.
#[derive(Debug, PartialEq)]
struct StatedBinding {
    construct: String,
    method_id: String,
    parameters: BTreeMap<String, f64>,
    options: BTreeMap<String, String>,
}

fn stated(request: &AnalysisRequest) -> Vec<StatedBinding> {
    vec![
        StatedBinding {
            construct: "system_weight".into(),
            method_id: request.weighing.method_id.clone(),
            parameters: request.weighing.parameters.clone(),
            options: request.weighing.options.clone(),
        },
        StatedBinding {
            construct: "movement_onset".into(),
            method_id: request.onset.method_id.clone(),
            parameters: request.onset.parameters.clone(),
            options: request.onset.options.clone(),
        },
        StatedBinding {
            construct: "takeoff".into(),
            method_id: request.takeoff.method_id.clone(),
            parameters: request.takeoff.parameters.clone(),
            options: request.takeoff.options.clone(),
        },
    ]
}

#[test]
fn a_request_writes_a_document_that_reads_back_as_the_same_request() {
    let request = owen_request();
    let original = document_of(&request);
    let json = serde_json::to_string_pretty(&original).expect("the document serialises");
    println!("{json}");

    let read: MethodSet = serde_json::from_str(&json).expect("the document parses");
    assert_eq!(read, original, "the document did not survive JSON");

    let resolved = read.resolve().expect("every construct has a slot");
    assert_eq!(
        stated(&resolved),
        stated(&request),
        "a value the caller stated did not come back"
    );
    assert_eq!(
        original.bindings.len(),
        3,
        "every stated slot is written out"
    );
}

#[test]
fn a_slot_nobody_stated_is_absent_rather_than_written_empty() {
    // Owen et al. state a weighing window and an onset criterion and no takeoff threshold,
    // so a document from that pipeline binds two constructs. An empty third would claim
    // somebody chose it.
    let mut request = owen_request();
    request.takeoff = MethodChoice::default();
    let document = document_of(&request);
    assert_eq!(document.bindings.len(), 2);
    assert!(document
        .bindings
        .iter()
        .all(|binding| binding.construct != "takeoff"));
}

#[test]
fn a_misspelt_field_is_refused_rather_than_dropped() {
    let mut json: serde_json::Value =
        serde_json::to_value(document_of(&owen_request())).expect("the document serialises");
    json["bindings"][0]["parameter"] = serde_json::json!({ "duration": 1.0 });
    let error = serde_json::from_value::<MethodSet>(json).expect_err("an unknown field is refused");
    println!("{error}");
    assert!(error.to_string().contains("parameter"));
}

#[test]
fn a_construct_this_build_runs_no_slot_for_is_refused_by_name() {
    let mut document = document_of(&owen_request());
    document.bindings.push(MethodSetBinding {
        construct: "phase_model".into(),
        method_id: "phase.some_rule".into(),
        parameters: BTreeMap::new(),
        options: BTreeMap::new(),
    });
    let refusal = document
        .resolve()
        .expect_err("an unrunnable construct is refused");
    println!("{}", refusal.message());
    assert!(refusal.message().contains("phase_model"));
    // What it could have asked for instead, so the reader is not left guessing.
    assert!(refusal
        .available
        .iter()
        .any(|name| name == "movement_onset"));
}

#[test]
fn a_document_from_a_later_version_is_refused_as_a_version() {
    let mut document = document_of(&owen_request());
    document.schema = "plateforce.method-set/2".into();
    let refusal = document.resolve().expect_err("a later schema is refused");
    println!("{}", refusal.message());
    // The reader is told to upgrade rather than handed a list of things to ask for
    // instead: there is nothing else this file could have been.
    assert!(refusal.message().contains("plateforce.method-set/2"));
    assert!(refusal.message().contains(METHOD_SET_SCHEMA));
    assert!(document.readable().is_err());
}

#[test]
fn the_schema_string_is_the_one_that_gets_committed_to_other_peoples_repositories() {
    assert_eq!(METHOD_SET_SCHEMA, "plateforce.method-set/1");
    assert_eq!(document_of(&owen_request()).schema, METHOD_SET_SCHEMA);
}
