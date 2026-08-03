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
        derived,
        conditioning,
        body_mass_kilograms,
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
        // Keyed by construct and already ordered, because the map is a `BTreeMap`: two runs
        // stating one set of rules in two orders are one request and fingerprint alike.
        "derived": derived
            .iter()
            .map(|(construct, choice)| (construct.clone(), method_choice(choice)))
            .collect::<serde_json::Map<String, Value>>(),
        // What the signal was conditioned with before anything was measured on it. Two runs
        // under different filters are two results, and a fingerprint blind to this would
        // call them one.
        "conditioning": conditioning
            .iter()
            .map(|(construct, choice)| (construct.clone(), method_choice(choice)))
            .collect::<serde_json::Map<String, Value>>(),
        "body_mass_kilograms": body_mass_kilograms,
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

    fn run_row() -> RunRow {
        RunRow {
            plateforce_version: "0.1.0".to_string(),
            registry_version: String::new(),
            registry_digest: "content-0".to_string(),
            request_digest: "content-1".to_string(),
            files_found: 6,
            files_unidentified: 0,
            trial_count: 6,
            computed_count: 6,
            refusal_count: 0,
            acquisition_complete_count: 0,
            trials_excluded: 0,
            gates_reporting: 0,
            gates_applied: 0,
            distinct_provenance_count: 2,
            trial_identity: "file_stem".to_string(),
            delimiter: "\t".to_string(),
            force_column_index: 0,
            sample_rate_hz: 1200.0,
            sentinel: String::new(),
            sentinel_rows_dropped: 0,
            run_fingerprint: String::new(),
        }
    }

    fn ids(first: &str, second: &str) -> BTreeSet<String> {
        [first.to_string(), second.to_string()]
            .into_iter()
            .collect()
    }

    /// How many distinct chains a run held is already a field on the row, so only the chains
    /// themselves separate two runs that walked the same files under the same request and
    /// resolved different methods on them.
    #[test]
    fn two_runs_that_ran_different_rules_the_same_number_of_ways_differ() {
        let row = run_row();
        let left = run_fingerprint(&row, &ids("content-aaa", "content-bbb"));
        let right = run_fingerprint(&row, &ids("content-ccc", "content-ddd"));
        assert_ne!(
            left, right,
            "these two runs held the same number of chains and not the same chains"
        );
    }

    #[test]
    fn one_run_fingerprints_the_same_way_twice() {
        let row = run_row();
        assert_eq!(
            run_fingerprint(&row, &ids("content-aaa", "content-bbb")),
            run_fingerprint(&row, &ids("content-bbb", "content-aaa"))
        );
    }
}
