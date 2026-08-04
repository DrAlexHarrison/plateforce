//! The rule that turned the landmarks into a number records what it read, in a folder run.
//!
//! A quantity carries the id of its arithmetic in `computed_by` and never in
//! `contributing_method_ids`. The chain a folder run wrote was built from the contributing
//! list alone, so the arithmetic reached `provenance.csv` as a bare id: the gravity behind the
//! flight-time height and the four integration choices behind every impulse figure were in the
//! terminal's record and in no folder run's. A number that moves when a value moves, with
//! nothing in the record naming the value, is the founding observation of this project
//! reproduced on our own batch surface.

mod common;

use std::collections::BTreeSet;

use plateforce_analysis::MethodChoice;

/// Every parameter the analysis bound to the rule named in `computed_by`, as
/// `(quantity, method_id, parameter, value)`.
///
/// Built by calling `plateforce_analysis::run` directly rather than by reading the batch back,
/// so this is two paths over one recording rather than a set compared with itself.
fn expected_from_the_analysis(
    trial: &plateforce_core::Trial,
    request: &plateforce_analysis::AnalysisRequest,
) -> BTreeSet<(String, String, String, String)> {
    let response = plateforce_analysis::run(trial, request).expect("the request is well formed");
    let mut expected = BTreeSet::new();
    for metric in &response.metrics {
        if metric.value.is_none() {
            continue;
        }
        let Some(arithmetic) = &metric.computed_by else {
            continue;
        };
        let Some(bound) = response
            .bound_methods
            .iter()
            .find(|bound| bound.method_id == *arithmetic)
        else {
            continue;
        };
        for (parameter, value) in &bound.bound_parameters {
            expected.insert((
                metric.key.to_string(),
                arithmetic.to_string(),
                parameter.clone(),
                value.clone(),
            ));
        }
    }
    expected
}

/// The same four fields off the relation the folder run wrote.
fn written_by_the_batch(
    result: &plateforce_batch::BatchResult,
) -> BTreeSet<(String, String, String, String)> {
    result
        .provenance
        .iter()
        .filter(|row| !row.parameter.is_empty())
        .map(|row| {
            (
                row.quantity.clone(),
                row.method_id.clone(),
                row.parameter.clone(),
                row.value.clone(),
            )
        })
        .collect()
}

fn one_trial() -> (plateforce_core::Trial, std::path::PathBuf) {
    let directory = common::tempdir("arithmetic-record");
    let generated = plateforce_batch::synthetic::write_corpus(&directory, 1, 1, 41)
        .expect("the corpus is written");
    let force: Vec<f64> = generated[0]
        .text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| line.trim().parse().expect("a number per line"))
        .collect();
    let trial = plateforce_core::Trial::new(force, 1200.0).expect("a trial");
    (trial, directory)
}

/// A request binding one rule computed from the landmarks, which is where the arithmetic that
/// reads a stated value lives.
fn request_binding_peak_force(
    averaging_window_seconds: f64,
) -> plateforce_analysis::AnalysisRequest {
    let mut request = common::analysis_request(1.0);
    request.derived.insert(
        "analysis_window".to_string(),
        MethodChoice {
            method_id: "window_end.takeoff.detected".to_string(),
            ..Default::default()
        },
    );
    request.derived.insert(
        "peak_force".to_string(),
        MethodChoice {
            method_id: "force.peak.estimator".to_string(),
            parameters: [(
                "averaging_window_seconds".to_string(),
                averaging_window_seconds,
            )]
            .into_iter()
            .collect(),
            ..Default::default()
        },
    );
    request
}

/// Every value the analysis says the arithmetic read is in the folder run's record.
///
/// A subset rather than an equality, because the batch also records the landmark rules and
/// their operators, which is more than the arithmetic alone.
#[test]
fn every_value_the_arithmetic_read_reaches_the_relation_a_reader_opens() {
    let (trial, directory) = one_trial();
    let request = request_binding_peak_force(0.05);
    let expected = expected_from_the_analysis(&trial, &request);
    assert!(
        !expected.is_empty(),
        "the analysis binds values to the rules it names in computed_by"
    );

    let set = plateforce_batch::TrialSet::walk(
        &directory,
        &common::synthetic_format(),
        &common::declared_pattern(),
    )
    .expect("the corpus walks");
    let batch = plateforce_batch::BatchRequest::new(request).resolving(&[
        "system_weight",
        "movement_onset",
        "takeoff",
    ]);
    let result =
        plateforce_batch::analyse(&set, &batch, &common::registry()).expect("the run proceeds");
    let written = written_by_the_batch(&result);

    let missing: Vec<_> = expected.difference(&written).cloned().collect();
    println!(
        "{} values bound to an arithmetic rule, {} of them in the relation",
        expected.len(),
        expected.len() - missing.len()
    );
    assert!(
        missing.is_empty(),
        "absent from provenance.csv: {missing:?}"
    );

    let _ = std::fs::remove_dir_all(&directory);
}

/// The value the record names is the value the number was taken at.
///
/// Without this the first guard passes on a record that reports a value the rule did not use:
/// two runs at different settings would write different numbers and one record. The window is
/// swept rather than toggled, so a rule that ignored the value entirely fails on the numbers
/// and a rule that recorded a constant fails on the record.
#[test]
fn a_number_that_moves_with_a_stated_value_carries_that_value_in_the_record() {
    let (_, directory) = one_trial();
    let set = plateforce_batch::TrialSet::walk(
        &directory,
        &common::synthetic_format(),
        &common::declared_pattern(),
    )
    .expect("the corpus walks");

    let mut seen: Vec<(f64, f64, String)> = Vec::new();
    for window_seconds in [0.0, 0.05, 0.4] {
        let batch = plateforce_batch::BatchRequest::new(request_binding_peak_force(window_seconds))
            .resolving(&["system_weight", "movement_onset", "takeoff"]);
        let result =
            plateforce_batch::analyse(&set, &batch, &common::registry()).expect("the run proceeds");
        let peak = result.results[0]
            .values
            .get("peak_force_newtons")
            .copied()
            .flatten()
            .expect("the rule ran");
        let recorded = result
            .provenance
            .iter()
            .find(|row| {
                row.method_id == "force.peak.estimator"
                    && row.parameter == "averaging_window_seconds"
            })
            .map(|row| row.value.clone())
            .unwrap_or_else(|| {
                panic!("the record names no averaging window at {window_seconds} s")
            });
        seen.push((window_seconds, peak, recorded));
    }

    for (window_seconds, peak, recorded) in &seen {
        println!("window {window_seconds} s: peak {peak} N, record says {recorded}");
        assert_eq!(
            recorded.parse::<f64>().expect("a number"),
            *window_seconds,
            "the record names a window the run was not taken at"
        );
    }
    // A wider average cannot report a higher peak, and these three are far enough apart on
    // this trace to separate. A record that stayed still while the numbers moved is the
    // failure this pair exists to catch, and one that moves with them is the pass.
    assert!(
        seen[0].1 > seen[1].1 && seen[1].1 > seen[2].1,
        "the window did not move the number: {seen:?}"
    );

    let _ = std::fs::remove_dir_all(&directory);
}
