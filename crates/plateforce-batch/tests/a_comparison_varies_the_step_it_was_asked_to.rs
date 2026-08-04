//! A folder comparison varies the step the caller named, and its record says which rule made
//! which number.
//!
//! A compare surface with the onset slot hardcoded reaches one construct of the thirteen
//! computed from the landmarks, the one the founding measurement happens to use. Reaching the
//! other twelve meets a second fault: a sweep reporting `method_ids` from the three landmark
//! fields alone returns three different jump heights under one chain and one `provenance_id`.
//! A record asserting sameness where the values disagree is worse than no record, so the two
//! are guarded together.

mod common;

use std::collections::BTreeSet;

use plateforce_analysis::MethodChoice;
use plateforce_batch::agreement::BatchCompareRequest;
use plateforce_batch::{axis_over, compare, BatchRequest};

/// A stamp with all three facts filled and each one different, so a record that transposed
/// two of them fails rather than matching itself.
fn a_registry_stamp() -> plateforce_core::provenance::RegistryStamp {
    plateforce_core::provenance::RegistryStamp::unpinned(
        Some("declared-2026-07-25".to_string()),
        Some("content-registry".to_string()),
    )
    .pinned_to(Some("pinned-2026.08.01".to_string()))
}

/// Every member of the block, so the run fingerprints rather than being withheld.
fn a_recorded_plate(floor: &str) -> plateforce_core::Acquisition {
    let mut acquisition = plateforce_core::Acquisition::default();
    for (member, value) in [
        ("filter_at_capture", "none"),
        ("tare_state", "zeroed_before_each_trial"),
        ("plate_natural_frequency_hz", "400"),
        ("floor_surface", floor),
        ("firmware_version", "3.1.4"),
    ] {
        acquisition
            .set_member(member, value)
            .expect("every member is one the block declares");
    }
    acquisition
}

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
            axis,
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

/// The regression guard proper. One folder, three steps, and each answer has to be the one
/// the caller asked for.
///
/// A surface with the step pinned answers alike whatever the rules name, so the assertion is a
/// relation between three runs rather than a number written here: the three sets of values
/// differ from each other, and each record names the construct its own rules are filed under.
/// Pinned to onset, the weighing and takeoff runs would hand weighing and takeoff rules to the
/// onset slot, which returns either the onset answer or no answer, and both fail below.
///
/// The subject 01 fixtures rather than the synthetic corpus, because the rules have to
/// genuinely disagree for the comparison of the three sets to mean anything. Two takeoff rules
/// that agree read exactly like a sweep that never reached the takeoff slot.
#[test]
fn the_step_a_comparison_sweeps_follows_the_rules_it_was_given() {
    let directory = common::tempdir("compare-three-steps");
    let copied = common::copy_committed_fixtures(&directory);
    let set = plateforce_batch::TrialSet::walk(
        &directory,
        &common::committed_format(),
        &plateforce_batch::TrialIdentity::FileStem,
    )
    .expect("the fixtures walk");

    let steps = [
        ("system_weight", "bwepoch.adaptive_lowest_variance"),
        ("movement_onset", "onset.threshold.absolute_force"),
        ("takeoff", "takeoff.threshold.flight_noise_k_sd"),
    ];
    let mut per_step: Vec<(&str, BTreeSet<String>)> = Vec::new();
    for (construct, rule) in steps {
        let result = run(
            &set,
            request_binding_takeoff_frame(),
            &[rule],
            "jump_height_from_takeoff_meters",
        );
        assert_eq!(
            result.construct, construct,
            "a {rule} comparison swept {} instead",
            result.construct
        );
        let record = result.run_row(&a_registry_stamp(), "content-request");
        assert_eq!(record.construct, construct, "and the record says so");
        // The swept step is not among the held, and the other two are. A record naming one
        // step under both headings describes a sweep nobody ran.
        let held: BTreeSet<&str> = record
            .held_fixed
            .iter()
            .map(|rule| rule.construct.as_str())
            .collect();
        assert!(!held.contains(construct), "{construct} is held and swept");
        for (other, _) in steps.iter().filter(|(name, _)| *name != construct) {
            assert!(held.contains(other), "{other} moved on a {construct} sweep");
        }

        let values: BTreeSet<String> = result
            .paired
            .iter()
            .filter_map(|row| row.value.map(|value| format!("{value:.9}")))
            .collect();
        assert!(
            result.paired.len() == 2 * result.trial_count && !values.is_empty(),
            "{construct}: {} rows over {} trials",
            result.paired.len(),
            result.trial_count
        );
        per_step.push((construct, values));
    }

    println!(
        "{copied} fixtures, distinct values per swept step: {:?}",
        per_step
            .iter()
            .map(|(name, values)| (*name, values.len()))
            .collect::<Vec<_>>()
    );
    for (index, (construct, values)) in per_step.iter().enumerate() {
        for (other, others) in per_step.iter().skip(index + 1) {
            assert_ne!(
                values, others,
                "sweeping {construct} and sweeping {other} returned the same numbers"
            );
        }
    }
    std::fs::remove_dir_all(&directory).ok();
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

/// A comparison that leaves the machine carries what produced it, not a pointer to it.
///
/// The two digests identify a run; they do not describe one. A reader holding this file and
/// nothing else has to be able to say which registry revision was cited, what the plate was,
/// and at what rate the traces were read, because none of it is recoverable from the numbers
/// and the recordings say none of it either.
///
/// Read back through the type rather than as text, so a field the writer stopped emitting is a
/// failure here rather than a key nobody notices is gone.
#[test]
fn a_comparison_carries_the_registry_and_the_plate_it_ran_under() {
    let (directory, set) = corpus();
    let named = vec!["onset.threshold.absolute_force".to_string()];
    let analysis = request_binding_takeoff_frame();
    let axis = axis_over(&analysis, &named).expect("the axis resolves");
    let batch = BatchRequest::new(analysis)
        .resolving(&["system_weight", "movement_onset", "takeoff"])
        .pinned_to(Some("pinned-2026.08.01".to_string()))
        .describing(a_recorded_plate("sprung_wood"));
    let result = compare(
        &set,
        &BatchCompareRequest {
            analysis: batch,
            axis,
            quantity: "jump_height_from_takeoff_meters".to_string(),
        },
    );

    let out = directory.join("out");
    result
        .write_csv(&out, &a_registry_stamp(), "content-request")
        .expect("the directory takes them");
    let record: plateforce_batch::agreement::CompareRunRow =
        serde_json::from_str(&std::fs::read_to_string(out.join("compare-run.json")).unwrap())
            .expect("the record reads back as the type that wrote it");

    // The caller's word and the registry's own, each under its own name. This pair has been
    // published transposed before, telling every reader the operator cited a revision no
    // operator had chosen.
    assert_eq!(
        record.registry_version.as_deref(),
        Some("pinned-2026.08.01")
    );
    assert_eq!(
        record.registry_declared_version.as_deref(),
        Some("declared-2026-07-25")
    );
    assert_eq!(record.registry_digest.as_deref(), Some("content-registry"));

    // The block itself, member by member, rather than the flag that says it was filled.
    assert_eq!(record.acquisition, a_recorded_plate("sprung_wood"));
    assert!(record.acquisition_complete);
    assert_eq!(record.format.sample_rate_hz, 1200.0);
    assert_eq!(record.format.force_column_index, 0);
    assert!(record.trial_identity.contains("declared_pattern"));

    // A filled block fingerprints, and the fingerprint answers to the block: one member
    // changed is a different run, which is the whole use a reader has for it.
    let published = record
        .run_fingerprint
        .clone()
        .expect("a filled block publishes a fingerprint");
    let mut under_another_floor = record.clone();
    under_another_floor.acquisition = a_recorded_plate("rubber_matting");
    under_another_floor.run_fingerprint = None;
    let moved = plateforce_batch::fingerprint::compare_run_fingerprint(
        &under_another_floor,
        &Default::default(),
    );
    assert_ne!(
        moved.published(),
        Some(published.as_str()),
        "two floors fingerprinted alike"
    );

    std::fs::remove_dir_all(&directory).ok();
}

/// A run nobody recorded the plate for carries no fingerprint at all.
///
/// Withheld rather than published incomplete: a digest over a half-filled block is a value two
/// labs can compare and must not, and a dataset that cannot fill the block fingerprints as
/// incomplete rather than as matching.
#[test]
fn a_comparison_with_no_plate_recorded_publishes_no_fingerprint() {
    let (directory, set) = corpus();
    let result = run(
        &set,
        request_binding_takeoff_frame(),
        &["onset.threshold.absolute_force"],
        "jump_height_from_takeoff_meters",
    );
    let record = result.run_row(&a_registry_stamp(), "content-request");
    println!(
        "no plate stated: complete {}, fingerprint {:?}",
        record.acquisition_complete, record.run_fingerprint
    );
    assert!(!record.acquisition_complete);
    assert_eq!(record.run_fingerprint, None);

    // The discriminator. The same run with the block filled does publish one, so the None
    // above is the completeness rule rather than a field nothing ever writes.
    let mut filled = record.clone();
    filled.acquisition = a_recorded_plate("sprung_wood");
    filled.acquisition_complete = filled.acquisition.is_complete();
    assert!(
        plateforce_batch::fingerprint::compare_run_fingerprint(&filled, &Default::default())
            .published()
            .is_some()
    );
    std::fs::remove_dir_all(&directory).ok();
}
