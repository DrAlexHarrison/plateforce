//! Identity within one run, and a change detector across runs.
//!
//! One digest implementation, the one already in the tree. Stated plainly because it will be
//! read as a stronger claim than it is: this identifies records and detects change, and it is
//! not a cryptographic commitment.

use std::collections::BTreeSet;

use plateforce_analysis::{AnalysisRequest, MethodChoice, WeighingChoice};
use plateforce_registry::content_digest;
use serde_json::{json, Value};

use crate::relations::{ProvenanceRow, RunRow};

/// Digest a record under the kind of record it is, so two kinds with identical bodies do not
/// collide.
fn digest(kind: &str, body: &Value) -> String {
    content_digest([(kind, body.to_string().as_str())])
}

/// The id every provenance row for one trial shares, taken over the chain itself so two
/// trials that ran the same way get one id and a per-trial override gets its own.
pub fn provenance_id(rows: &[ProvenanceRow]) -> String {
    let mut ordered: Vec<Value> = rows
        .iter()
        .map(|row| {
            json!({
                "quantity": row.quantity,
                "depth": row.depth,
                "method_id": row.method_id,
                "parameter": row.parameter,
                "value": row.value,
                "source": row.source,
            })
        })
        .collect();
    ordered.sort_by_key(|row| row.to_string());
    digest("provenance", &Value::Array(ordered))
}

/// Everything the request pins, in a canonical shape.
///
/// Destructured without a rest pattern on purpose: a field added to any of these three
/// request types stops this compiling rather than silently leaving the digest blind to it.
pub fn request_digest(request: &AnalysisRequest, registry_version: Option<&str>) -> String {
    let AnalysisRequest {
        weighing,
        onset,
        takeoff,
        touchdown_index,
        gravity_meters_per_second_squared,
        registry_backed_ids,
    } = request;
    let WeighingChoice {
        method_id: weighing_id,
        start_index,
        parameters: weighing_parameters,
        options: weighing_options,
        recommended: weighing_recommended,
        method_from_recommendation: weighing_method_from_recommendation,
        from_registry_default: weighing_from_registry_default,
    } = weighing;
    let mut backed = registry_backed_ids.clone();
    backed.sort();

    let body = json!({
        "weighing": {
            "method_id": weighing_id,
            "start_index": start_index,
            "parameters": weighing_parameters,
            "options": weighing_options,
            "recommended": weighing_recommended,
            "method_from_recommendation": weighing_method_from_recommendation,
            "from_registry_default": weighing_from_registry_default,
        },
        "onset": method_choice(onset),
        "takeoff": method_choice(takeoff),
        "touchdown_index": touchdown_index,
        "gravity_meters_per_second_squared": gravity_meters_per_second_squared,
        "registry_backed_ids": backed,
        "registry_version": registry_version,
    });
    digest("request", &body)
}

fn method_choice(choice: &MethodChoice) -> Value {
    let MethodChoice {
        method_id,
        parameters,
        options,
        manual_index,
        recommended,
        method_from_recommendation,
        from_registry_default,
    } = choice;
    json!({
        "method_id": method_id,
        "parameters": parameters,
        "options": options,
        "recommended": recommended,
        "method_from_recommendation": method_from_recommendation,
        "from_registry_default": from_registry_default,
        "manual_index": manual_index,
    })
}

/// The run's own identity: everything in the `run` row except this field, plus the distinct
/// provenance ids, so a run whose trials did not all run the same way fingerprints
/// differently from one where they did.
pub fn run_fingerprint(run: &RunRow, provenance_ids: &BTreeSet<String>) -> String {
    let mut without_itself = run.clone();
    without_itself.run_fingerprint = String::new();
    let body = json!({
        "run": serde_json::to_value(&without_itself).unwrap_or(Value::Null),
        "provenance_ids": provenance_ids.iter().collect::<Vec<_>>(),
    });
    digest("run", &body)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(method_id: &str, value: &str) -> ProvenanceRow {
        ProvenanceRow {
            provenance_id: String::new(),
            quantity: "jump_height_from_takeoff_meters".to_string(),
            depth: 1,
            method_id: method_id.to_string(),
            parameter: "k".to_string(),
            value: value.to_string(),
            source: "stated".to_string(),
        }
    }

    #[test]
    fn one_chain_gets_one_id_however_the_rows_are_ordered() {
        let forward = vec![row("onset.a", "5"), row("onset.b", "3")];
        let reversed = vec![row("onset.b", "3"), row("onset.a", "5")];
        assert_eq!(provenance_id(&forward), provenance_id(&reversed));
    }

    #[test]
    fn a_changed_parameter_is_a_different_chain() {
        assert_ne!(
            provenance_id(&[row("onset.a", "5")]),
            provenance_id(&[row("onset.a", "3")])
        );
    }

    #[test]
    fn the_digest_carries_the_shape_the_registry_already_uses() {
        let id = provenance_id(&[row("onset.a", "5")]);
        assert!(id.starts_with("content-"), "{id}");
        assert_eq!(id.len(), "content-".len() + 16);
    }
}
