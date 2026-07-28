//! The request the interface posts, pinned as text.
//!
//! `web/app.js` builds this object and the module parses it. A field renamed on one side
//! and not the other loses every value it carried, each rule falls back to its own, and
//! the answer still comes back looking reasonable. So the shape is stated here in the same
//! form it crosses the boundary in, and each rule is asked what it did not read.

use plateforce_wasm::analysis::{run, AnalysisRequest};
use plateforce_wasm::demo::synthetic_countermovement_jump;

/// The recommended opening selection, with each rule's parameters under the names the
/// registry publishes for it.
const RECOMMENDED: &str = r#"{
  "weighing": {
    "method_id": "bwepoch.adaptive_lowest_variance",
    "start_index": null,
    "parameters": { "window_seconds": 1.0, "variance_floor_pct_bodyweight": 0.5 },
    "options": {}
  },
  "onset": {
    "method_id": "onset.threshold.noise_relative",
    "parameters": { "k": 5.0 },
    "options": {},
    "manual_index": null
  },
  "takeoff": {
    "method_id": "takeoff.threshold.absolute_force",
    "parameters": { "threshold_n": 20.0, "persistence_ms": 15.0 },
    "options": {},
    "manual_index": null
  },
  "touchdown_index": null,
  "gravity_meters_per_second_squared": 9.80665,
  "registry_backed_ids": ["bwepoch.adaptive_lowest_variance", "onset.threshold.noise_relative", "takeoff.threshold.absolute_force"]
}"#;

/// A window dragged on the trace: the by-hand rule, its own span, and a start index.
const WINDOW_PLACED_BY_HAND: &str = r#"{
  "weighing": {
    "method_id": "bwepoch.manual_placement",
    "start_index": 1200,
    "parameters": { "span_seconds": 0.7345 },
    "options": {}
  },
  "onset": {
    "method_id": "onset.threshold.relative_to_system_weight",
    "parameters": { "pct": 2.5 },
    "options": {},
    "manual_index": null
  },
  "takeoff": {
    "method_id": "takeoff.threshold.flight_noise_k_sd",
    "parameters": { "k": 5.0 },
    "options": {},
    "manual_index": null
  },
  "touchdown_index": null,
  "gravity_meters_per_second_squared": 9.80665,
  "registry_backed_ids": []
}"#;

#[test]
fn every_value_the_interface_posts_is_read_by_the_rule_it_was_posted_for() {
    let trial = synthetic_countermovement_jump();
    for payload in [RECOMMENDED, WINDOW_PLACED_BY_HAND] {
        let request: AnalysisRequest =
            serde_json::from_str(payload).expect("the interface's request no longer parses");
        let response = run(&trial, &request).expect("the interface's request no longer runs");
        for method in &response.bound_methods {
            assert!(
                method.unread_parameters.is_empty(),
                "{} was posted {:?}, which it does not read",
                method.method_id,
                method.unread_parameters
            );
        }
    }
}

/// A field name that drifted apart is refused, rather than the values it carried being
/// dropped and every rule falling back to its own.
#[test]
fn a_field_this_module_does_not_carry_is_refused() {
    let drifted = RECOMMENDED.replace(
        "\"parameters\": { \"k\": 5.0 }",
        "\"values\": { \"k\": 5.0 }",
    );
    assert!(serde_json::from_str::<AnalysisRequest>(&drifted).is_err());
}
