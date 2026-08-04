//! A folder comparison varies the step the caller named, and its record says which rule made
//! which number.
//!
//! The compare surface hardcoded the onset slot, so one construct of the thirteen computed from
//! the landmarks could be compared over a folder, and it was the one the founding measurement
//! happens to use. Reaching the other twelve exposed a second defect immediately: the sweep
//! reports `method_ids` from the three landmark fields alone, so three different jump heights
//! came back under one chain and one `provenance_id`. A record asserting sameness where the
//! values disagree is worse than no record, so the two are guarded together.

mod common;

use std::collections::BTreeSet;

use plateforce_analysis::MethodChoice;
use plateforce_batch::agreement::BatchCompareRequest;
use plateforce_batch::{axis_over, compare, BatchRequest};

/// Four trials of one subject, so a comparison has a denominator larger than one.
fn corpus() -> (std::path::PathBuf, plateforce_batch::TrialSet) {
    let directory = common::tempdir("compare-axis");
    plateforce_batch::synthetic::write_corpus(&directory, 1, 4, 23).expect("the corpus is written");
    let set = plateforce_batch::TrialSet::walk(
        &directory,
        &common::synthetic_format(),
        &common::declared_pattern(),
    )
    .expect("the corpus walks");
    (directory, set)
}

fn request_binding_takeoff_frame() -> plateforce_analysis::AnalysisRequest {
    let mut analysis = common::analysis_request(1.0);
    analysis.derived.insert(
        "jump_height.takeoff_frame".to_string(),
        MethodChoice {
            method_id: "jumpheight.takeoff.impulse_momentum".to_string(),
            ..Default::default()
        },
    );
    analysis
}

fn run(
    set: &plateforce_batch::TrialSet,
    analysis: plateforce_analysis::AnalysisRequest,
    against: &[&str],
    quantity: &str,
) -> plateforce_batch::BatchCompareResult {
    let named: Vec<String> = against.iter().map(|id| (*id).to_string()).collect();
    let axis = axis_over(&analysis, &named).expect("the axis resolves");
    let batch =
        BatchRequest::new(analysis).resolving(&["system_weight", "movement_onset", "takeoff"]);
    compare(
        set,
        &BatchCompareRequest {
            analysis: batch,
            slot: axis.slot,
            method_ids: axis.method_ids,
            quantity: quantity.to_string(),
        },
    )
}

/// The rules that turn a takeoff into a height are compared over a folder, which no surface
/// could do. Three rules, four trials, twelve rows, and the values genuinely differ.
#[test]
fn a_construct_computed_from_the_landmarks_is_an_axis_a_folder_can_sweep() {
    let (directory, set) = corpus();
    let result = run(
        &set,
        request_binding_takeoff_frame(),
        &[
            "jumpheight.takeoff.work_energy",
            "jumpheight.takeoff.peak_velocity.chavda2018",
        ],
        "jump_height_from_takeoff_meters",
    );

    assert_eq!(result.slot, "jump_height.takeoff_frame");
    assert_eq!(result.method_ids.len(), 3, "{:?}", result.method_ids);
    assert_eq!(result.paired.len(), 12, "3 rules over 4 trials");

    // A sweep that ran three rules and got one number three times has not compared anything,
    // which is what a slot that never reached the derived map would produce.
    let values: BTreeSet<String> = result
        .paired
        .iter()
        .filter(|row| row.trial_id.ends_with('1'))
        .filter_map(|row| row.value.map(|value| format!("{value:.9}")))
        .collect();
    println!("values on one trial across the three rules: {values:?}");
    assert!(
        values.len() >= 2,
        "three rules produced {} distinct values",
        values.len()
    );

    let _ = std::fs::remove_dir_all(&directory);
}

/// The defect this nearly shipped. Two variants differing only in a rule computed from the
/// landmarks arrived with identical `method_ids`, so they were keyed to one chain and one
/// `provenance_id`, and the export said three different numbers came from the same rules.
///
/// Asserted as a relation between two sets rather than as a count: every distinct value on a
/// trial has a distinct chain, and every chain names the rule the variant ran.
#[test]
fn variants_that_differ_only_in_a_derived_rule_carry_different_chains() {
    let (directory, set) = corpus();
    let bound = "jumpheight.takeoff.impulse_momentum";
    let against = [
        "jumpheight.takeoff.work_energy",
        "jumpheight.takeoff.peak_velocity.chavda2018",
    ];
    let result = run(
        &set,
        request_binding_takeoff_frame(),
        &against,
        "jump_height_from_takeoff_meters",
    );

    let one_trial: Vec<_> = result
        .paired
        .iter()
        .filter(|row| row.trial_id == result.paired[0].trial_id)
        .collect();
    assert_eq!(one_trial.len(), 3, "three rules on one trial");

    let chains: BTreeSet<&str> = one_trial
        .iter()
        .map(|row| row.provenance_id.as_str())
        .collect();
    println!(
        "{} rows on one trial, {} distinct chains",
        one_trial.len(),
        chains.len()
    );
    assert_eq!(
        chains.len(),
        one_trial.len(),
        "three rules, {} chains: a reader cannot tell which rule made which number",
        chains.len()
    );

    // And the chain names the rule rather than merely differing from its neighbours. A digest
    // that varied for any other reason would satisfy the count above and nothing else.
    for row in &one_trial {
        let named: Vec<&str> = result
            .provenance
            .iter()
            .filter(|entry| entry.provenance_id == row.provenance_id)
            .map(|entry| entry.method_id.as_str())
            .collect();
        let expected = [bound, against[0], against[1]]
            .into_iter()
            .find(|id| row.variant_label.contains(id))
            .expect("the label names the rule the variant ran");
        assert!(
            named.contains(&expected),
            "the chain for {} names {named:?} and not {expected}",
            row.variant_label
        );
    }

    let _ = std::fs::remove_dir_all(&directory);
}

/// The control, and the regression guard. An onset comparison is what this surface already did,
/// and it has to keep doing it identically: the axis is now read off the rules rather than
/// hardcoded, so the old behaviour has to fall out of the new path rather than be preserved
/// beside it.
#[test]
fn an_onset_comparison_still_varies_onset_and_names_it() {
    let (directory, set) = corpus();
    let result = run(
        &set,
        common::analysis_request(1.0),
        &["onset.threshold.absolute_force"],
        "jump_height_from_takeoff_meters",
    );

    assert_eq!(result.slot, "onset");
    assert_eq!(
        result.method_ids,
        vec![
            "onset.threshold.noise_relative".to_string(),
            "onset.threshold.absolute_force".to_string()
        ]
    );
    assert_eq!(result.paired.len(), 8, "2 rules over 4 trials");
    let chains: BTreeSet<&str> = result
        .paired
        .iter()
        .filter(|row| row.trial_id == result.paired[0].trial_id)
        .map(|row| row.provenance_id.as_str())
        .collect();
    assert_eq!(
        chains.len(),
        2,
        "the two onset rules were already distinguished"
    );

    let _ = std::fs::remove_dir_all(&directory);
}
