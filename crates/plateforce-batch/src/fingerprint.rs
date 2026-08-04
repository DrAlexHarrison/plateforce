//! Identity within one run, and a change detector across runs.
//!
//! One digest implementation, the one already in the tree. Stated plainly because it will be
//! read as a stronger claim than it is: this identifies records and detects change, and it is
//! not a cryptographic commitment.

use std::collections::BTreeSet;

use plateforce_analysis::{AnalysisRequest, MethodChoice, WeighingChoice};
use plateforce_core::reporting::Fingerprint;
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
        gravity_source,
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
        method_from_registry_default: weighing_method_from_registry_default,
        from_registry_default: weighing_from_registry_default,
        cited: weighing_cited,
        preset: weighing_preset,
    } = weighing;
    let mut backed = registry_backed_ids.clone();
    backed.sort();

    // The pipeline pins the digest for the reason `recommended` does: one set of numbers
    // reached by adopting a published pipeline and one reached by typing them carry
    // different records, and the record is what this identifies.
    let body = json!({
        "weighing": {
            "method_id": weighing_id,
            "start_index": start_index,
            "parameters": weighing_parameters,
            "options": weighing_options,
            "recommended": weighing_recommended,
            "method_from_recommendation": weighing_method_from_recommendation,
            // Pinned for the reason the line above is: a rule the caller picked and one the
            // registry declared for a construct nobody named run the same arithmetic and
            // leave two different records.
            "method_from_registry_default": weighing_method_from_registry_default,
            "from_registry_default": weighing_from_registry_default,
            "cited": weighing_cited,
            "preset": weighing_preset,
        },
        "onset": method_choice(onset),
        "takeoff": method_choice(takeoff),
        "touchdown_index": touchdown_index,
        "gravity_meters_per_second_squared": gravity_meters_per_second_squared,
        // Pinned for the reason `recommended` is: one run whose author chose this gravity and
        // one that took the constant the request type fills in produce the same number under
        // different records, and the record is what this identifies.
        "gravity_source": gravity_source,
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
        method_from_registry_default,
        from_registry_default,
        cited,
        preset,
    } = choice;
    json!({
        "method_id": method_id,
        "parameters": parameters,
        "options": options,
        "recommended": recommended,
        "method_from_recommendation": method_from_recommendation,
        "method_from_registry_default": method_from_registry_default,
        "from_registry_default": from_registry_default,
        "cited": cited,
        "preset": preset,
        "manual_index": manual_index,
    })
}

/// The run's own identity: everything in the `run` row except this field, plus the distinct
/// provenance ids, so a run whose trials did not all run the same way fingerprints
/// differently from one where they did.
///
/// Returned as `plateforce_core::reporting::Fingerprint` rather than as a digest string, so
/// the rule that an unfilled acquisition block never matches is the one core already
/// implements rather than a second copy of it here. The row carries the acquisition block
/// whole, so the digest covers the plate and its settings and not merely a count of the
/// trials they applied to.
pub fn run_fingerprint(run: &RunRow, provenance_ids: &BTreeSet<String>) -> Fingerprint {
    let mut without_itself = run.clone();
    without_itself.run_fingerprint = None;
    // The saved plate goes with it. A profile is a way of not retyping the members, and the
    // members are already here; hashing the name a lab files them under would make two labs
    // whose plates are configured identically fail to match over a nickname.
    without_itself.plate_profile = None;
    fingerprint_of(
        "run",
        &without_itself,
        run.acquisition_complete,
        provenance_ids,
    )
}

/// The same identity for a comparison, which describes what it varied as well as what it read.
///
/// A separate kind word, so a comparison and an analysis over one folder under one plate
/// cannot digest alike. What goes in and what is dropped is the analysing run's rule, applied
/// once below rather than restated here.
pub fn compare_run_fingerprint(
    run: &crate::agreement::CompareRunRow,
    provenance_ids: &BTreeSet<String>,
) -> Fingerprint {
    let mut without_itself = run.clone();
    without_itself.run_fingerprint = None;
    without_itself.plate_profile = None;
    fingerprint_of(
        "compare_run",
        &without_itself,
        run.acquisition_complete,
        provenance_ids,
    )
}

/// A record's identity, and whether it may be published at all.
///
/// The one place the completeness rule reaches a digest. `Fingerprint` withholds an incomplete
/// one, so a record whose acquisition block was never filled carries nothing that could be
/// compared, and neither kind of run gets its own answer to that.
fn fingerprint_of<T: serde::Serialize>(
    kind: &str,
    row: &T,
    acquisition_complete: bool,
    provenance_ids: &BTreeSet<String>,
) -> Fingerprint {
    let body = json!({
        "run": serde_json::to_value(row).unwrap_or(Value::Null),
        "provenance_ids": provenance_ids.iter().collect::<Vec<_>>(),
    });
    Fingerprint {
        complete: acquisition_complete,
        digest: digest(kind, &body),
    }
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

    /// A filled acquisition block, so the comparisons below are over the digest.
    ///
    /// An incomplete `Fingerprint` matches nothing, so `assert_ne!` between two of them
    /// passes whatever the digests are.
    fn a_recorded_plate() -> plateforce_core::Acquisition {
        plateforce_core::Acquisition {
            filter_at_capture: Some("none".to_string()),
            tare_state: Some("tared_before_trial".to_string()),
            plate_natural_frequency_hz: Some(400.0),
            floor_surface: Some("concrete".to_string()),
            firmware_version: Some("2.4.1".to_string()),
        }
    }

    fn run_row() -> RunRow {
        RunRow {
            plateforce_version: "0.1.0".to_string(),
            registry_version: None,
            registry_declared_version: None,
            registry_digest: "content-0".to_string(),
            request_digest: "content-1".to_string(),
            files_found: 6,
            files_without_declared_suffix: 0,
            files_unidentified: 0,
            trial_count: 6,
            computed_count: 6,
            refusal_count: 0,
            acquisition_complete_count: 6,
            acquisition: a_recorded_plate(),
            acquisition_complete: true,
            plate_profile: None,
            trials_excluded: 0,
            gates_reporting: 0,
            gates_applied: 0,
            distinct_provenance_count: 2,
            trial_identity: "file_stem".to_string(),
            delimiter: "\t".to_string(),
            force_column_index: 0,
            sample_rate_hz: 1200.0,
            sentinel: String::new(),
            samples_matching_the_convention: 0,
            samples_carrying_no_number: 0,
            run_fingerprint: None,
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

    /// The block is inside the digest rather than beside it, so two runs off differently
    /// configured plates are two runs. A row carrying only `acquisition_complete_count` would
    /// have made these one.
    #[test]
    fn two_runs_off_different_plates_are_two_runs() {
        let one_plate = run_row();
        let mut another_plate = run_row();
        another_plate.acquisition.floor_surface = Some("sprung".to_string());

        assert_ne!(
            run_fingerprint(&one_plate, &ids("content-aaa", "content-bbb")).digest,
            run_fingerprint(&another_plate, &ids("content-aaa", "content-bbb")).digest,
            "the acquisition block did not reach the digest"
        );
    }

    /// Two labs that recorded the same five answers match, whatever they call their plates.
    ///
    /// Paired with a control that moves a member rather than a name, because a digest blind to
    /// the whole row would pass the first half of this and fail nothing: the two rows below
    /// differ only in the name, and `two_runs_off_different_plates_are_two_runs` above proves
    /// the same digest still sees the members.
    #[test]
    fn what_a_lab_calls_its_plate_is_not_part_of_the_digest() {
        let mut here = run_row();
        here.plate_profile = Some(plateforce_core::PlateProfileAttribution {
            name: "lab-kistler-1".to_string(),
            revision: plateforce_core::PlateProfileAttribution::revision_of(&here.acquisition),
            superseded_members: std::collections::BTreeMap::new(),
        });
        let mut elsewhere = run_row();
        elsewhere.plate_profile = Some(plateforce_core::PlateProfileAttribution {
            name: "the-blue-one".to_string(),
            revision: plateforce_core::PlateProfileAttribution::revision_of(&elsewhere.acquisition),
            superseded_members: std::collections::BTreeMap::new(),
        });

        assert_eq!(
            run_fingerprint(&here, &ids("content-aaa", "content-bbb")).digest,
            run_fingerprint(&elsewhere, &ids("content-aaa", "content-bbb")).digest,
            "a nickname reached the digest, so two labs with one plate configuration cannot match"
        );
        assert_eq!(
            run_fingerprint(&here, &ids("content-aaa", "content-bbb")).digest,
            run_fingerprint(&run_row(), &ids("content-aaa", "content-bbb")).digest,
            "naming a saved plate changed the digest of a run whose members did not move"
        );
    }

    /// A run nobody recorded the plate for publishes nothing to compare: it fingerprints as
    /// incomplete rather than as matching.
    ///
    /// Taken over two runs whose digests differ, so a `published` returning the digest would
    /// redden here rather than being satisfied by one value equalling itself.
    #[test]
    fn a_run_with_no_acquisition_block_publishes_no_digest() {
        let mut silent = run_row();
        silent.acquisition = plateforce_core::Acquisition::default();
        silent.acquisition_complete = false;
        silent.acquisition_complete_count = 0;

        let mut also_silent = silent.clone();
        also_silent.trial_count = 7;
        also_silent.files_found = 7;

        let left = run_fingerprint(&silent, &ids("content-aaa", "content-bbb"));
        let right = run_fingerprint(&also_silent, &ids("content-aaa", "content-bbb"));

        assert_ne!(left.digest, right.digest, "these two runs read two folders");
        assert_eq!(left.published(), None);
        assert_eq!(right.published(), None);
    }

    /// The other side of the guard above, so it cannot be met by publishing nothing for
    /// everything.
    #[test]
    fn a_run_that_recorded_its_plate_publishes_its_digest() {
        let row = run_row();
        let taken = run_fingerprint(&row, &ids("content-aaa", "content-bbb"));

        assert_eq!(taken.published(), Some(taken.digest.as_str()));
    }
}
