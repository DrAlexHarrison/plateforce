//! A rule a run cannot bind is refused once, before the first file is opened.
//!
//! The analysis refuses a construct with no rule behind it per trial, which is right for one
//! trial and wrong for a folder: a corpus of 244 answered one unmakeable binding with 244
//! identical rows and no statement of the fault. The terminal checks the line before it builds
//! the request; this is the check for every other caller, which builds a request directly.

mod common;

use plateforce_analysis::MethodChoice;
use plateforce_core::RefusalCode;

fn corpus_of(trials: usize) -> std::path::PathBuf {
    let directory = common::tempdir("unmakeable-binding");
    plateforce_batch::synthetic::write_corpus(&directory, 1, trials, 19)
        .expect("the corpus is written");
    directory
}

fn request_with(construct: &str, method_id: &str) -> plateforce_batch::BatchRequest {
    let mut analysis = common::analysis_request(1.0);
    analysis.derived.insert(
        construct.to_string(),
        MethodChoice {
            method_id: method_id.to_string(),
            ..Default::default()
        },
    );
    plateforce_batch::BatchRequest::new(analysis).resolving(&[
        "system_weight",
        "movement_onset",
        "takeoff",
    ])
}

/// A construct this build runs no rule for ends the run rather than every trial in it, and the
/// refusal lists the constructs that do run.
#[test]
fn a_construct_with_no_rule_behind_it_refuses_the_run_rather_than_each_trial() {
    let directory = corpus_of(4);
    let set = plateforce_batch::TrialSet::walk(
        &directory,
        &common::synthetic_format(),
        &common::declared_pattern(),
    )
    .expect("the corpus walks");
    assert_eq!(
        set.len(),
        4,
        "the denominator the refusal is not repeated over"
    );

    let refusal = plateforce_batch::analyse(
        &set,
        &request_with("not_a_construct", "anything"),
        &common::registry(),
    )
    .expect_err("a construct with no rule behind it is refused");
    println!("{}: {}", refusal.code.wire_name(), refusal.message);
    assert!(refusal.message.contains("phase_model"), "{refusal:?}");
    assert!(
        !matches!(refusal.code, RefusalCode::DecisionNotMade),
        "a name this build does not run is not an unmade choice: {refusal:?}"
    );

    let _ = std::fs::remove_dir_all(&directory);
}

/// An id that is a rule, named for a construct it is not filed under. Checking that the id
/// exists anywhere would bind an onset rule to peak force and carry its author's citation onto
/// a number that author's method did not produce.
#[test]
fn an_id_filed_under_another_construct_refuses_the_run() {
    let directory = corpus_of(4);
    let set = plateforce_batch::TrialSet::walk(
        &directory,
        &common::synthetic_format(),
        &common::declared_pattern(),
    )
    .expect("the corpus walks");

    let refusal = plateforce_batch::analyse(
        &set,
        &request_with("peak_force", "onset.threshold.absolute_force"),
        &common::registry(),
    )
    .expect_err("an id filed elsewhere is refused");
    println!("{}: {}", refusal.code.wire_name(), refusal.message);
    assert!(refusal.message.contains("force.peak."), "{refusal:?}");
    assert!(
        !refusal.message.contains("onset.threshold.noise_relative"),
        "the alternatives are the ones filed under the construct named: {refusal:?}"
    );

    let _ = std::fs::remove_dir_all(&directory);
}

/// The control the two above need: a binding this build can make reaches the trials and
/// produces the quantity, so a refusal is the check working rather than the run being broken.
#[test]
fn a_binding_this_build_can_make_is_not_refused() {
    let directory = corpus_of(4);
    let set = plateforce_batch::TrialSet::walk(
        &directory,
        &common::synthetic_format(),
        &common::declared_pattern(),
    )
    .expect("the corpus walks");

    let mut analysis = common::analysis_request(1.0);
    for (construct, method_id) in [
        ("analysis_window", "window_end.takeoff.detected"),
        ("peak_force", "force.peak.gross"),
    ] {
        analysis.derived.insert(
            construct.to_string(),
            MethodChoice {
                method_id: method_id.to_string(),
                ..Default::default()
            },
        );
    }
    let request = plateforce_batch::BatchRequest::new(analysis).resolving(&[
        "system_weight",
        "movement_onset",
        "takeoff",
    ]);
    let result =
        plateforce_batch::analyse(&set, &request, &common::registry()).expect("the run proceeds");
    let answered = result
        .results
        .iter()
        .filter(|row| {
            row.values
                .get("peak_force_newtons")
                .copied()
                .flatten()
                .is_some()
        })
        .count();
    println!(
        "peak force on {answered} of {} trials",
        result.results.len()
    );
    assert_eq!(answered, result.results.len());

    let _ = std::fs::remove_dir_all(&directory);
}
