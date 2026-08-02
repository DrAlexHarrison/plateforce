//! A reliability figure carries the interval it was taken over, or it does not exist.

mod common;

use common::{bound_request, declared_pattern, registry, synthetic_format, tempdir};
use plateforce_batch::agreement::{
    compare_coefficients, per_subject_values, reliability_coefficient_of_variation,
    reliability_icc, subject_coefficient_of_variation,
};
use plateforce_batch::{analyse, ReliabilityInterval, TrialSet};
use plateforce_core::agreement::IntraclassForm;
use plateforce_core::DispersionEstimator;

const HEIGHT: &str = "jump_height_from_takeoff_meters";

fn per_subject(name: &str, subjects: usize, trials: usize) -> (std::path::PathBuf, Vec<Vec<f64>>) {
    let directory = tempdir(name);
    plateforce_batch::synthetic::write_corpus(&directory, subjects, trials, 7).unwrap();
    let set = TrialSet::walk(&directory, &synthetic_format(), &declared_pattern()).unwrap();
    let result = analyse(&set, &bound_request(), &registry()).unwrap();
    assert_eq!(
        result.coverage.computed,
        subjects * trials,
        "every trial computed"
    );
    let values = per_subject_values(&set, &result, HEIGHT).expect("a declared pattern groups");
    (directory, values)
}

#[test]
fn interval_travels_with_every_reliability_figure() {
    let (directory, values) = per_subject("reliability-interval", 4, 4);

    let figure = reliability_coefficient_of_variation(
        &values,
        DispersionEstimator::Sample,
        ReliabilityInterval::WithinSession,
    )
    .expect("four subjects of four trials support it");

    println!(
        "cv {:.4} percent over {} subjects, interval {}",
        figure.figure().percent,
        figure.figure().n,
        figure.interval().as_registry_str()
    );
    assert_eq!(figure.interval(), ReliabilityInterval::WithinSession);
    assert_eq!(figure.figure().n, 4, "the count is subjects, not trials");
    std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn cv_is_taken_per_subject_then_averaged_across_subjects() {
    let (directory, values) = per_subject("reliability-cv", 3, 4);

    // The registry's rule, verbatim: for each subject the standard deviation over the mean,
    // then averaged across subjects. Checked by taking the same route by hand.
    let each: Vec<f64> = values
        .iter()
        .map(|subject| {
            subject_coefficient_of_variation(subject, DispersionEstimator::Sample)
                .unwrap()
                .percent
        })
        .collect();
    let by_hand = each.iter().sum::<f64>() / each.len() as f64;

    let figure = reliability_coefficient_of_variation(
        &values,
        DispersionEstimator::Sample,
        ReliabilityInterval::WithinSession,
    )
    .unwrap();
    println!(
        "per subject {:?}, averaged {:.6}, reported {:.6}",
        each.iter().map(|v| format!("{v:.4}")).collect::<Vec<_>>(),
        by_hand,
        figure.figure().percent
    );
    assert!((figure.figure().percent - by_hand).abs() < 1e-12);
    std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn two_coefficients_under_different_conventions_refuse_to_be_compared() {
    let (directory, values) = per_subject("reliability-conventions", 3, 4);

    let sample = reliability_coefficient_of_variation(
        &values,
        DispersionEstimator::Sample,
        ReliabilityInterval::WithinSession,
    )
    .unwrap();
    let population = reliability_coefficient_of_variation(
        &values,
        DispersionEstimator::Population,
        ReliabilityInterval::WithinSession,
    )
    .unwrap();

    // Two different numbers reported under one label, and the difference between the two
    // conventions has never been published, so the comparison is refused rather than warned.
    let refusal = compare_coefficients(sample.figure(), population.figure())
        .expect_err("the conventions differ");
    let message = refusal.message();
    println!("{message}");
    assert!(message.contains("sample"), "{message}");
    assert!(message.contains("population"), "{message}");

    let same = compare_coefficients(sample.figure(), sample.figure()).expect("one convention");
    assert_eq!(same, 0.0);
    std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn an_intraclass_figure_also_carries_its_interval() {
    let (directory, values) = per_subject("reliability-icc", 4, 3);
    let figure = reliability_icc(
        &values,
        IntraclassForm::AbsoluteAgreementSingle,
        ReliabilityInterval::BetweenSession,
    )
    .expect("a balanced set of four subjects by three trials");

    println!(
        "icc {:.6} over {} subjects x {} measurements, interval {}",
        figure.figure().value,
        figure.figure().subjects,
        figure.figure().measurements,
        figure.interval().as_registry_str()
    );
    assert_eq!(figure.interval(), ReliabilityInterval::BetweenSession);
    assert_eq!(figure.figure().subjects, 4);
    assert_eq!(figure.figure().measurements, 3);
    std::fs::remove_dir_all(&directory).ok();
}

// Task 5.7's compile-failure check. Uncomment and run
// `cargo build -p plateforce-batch --tests` to confirm the interval cannot be omitted: the
// error must name the missing argument rather than a type mismatch, because a field with a
// fallback would compile.
// Run once and recorded: the build fails with E0061, "argument #3 of type
// ReliabilityInterval is missing", which names the missing argument rather than a type
// mismatch, because a field carrying a fallback would have compiled.
//
// #[test]
// fn a_reliability_figure_without_an_interval_does_not_build() {
//     let (_directory, values) = per_subject("reliability-no-interval", 3, 3);
//     let _ = reliability_coefficient_of_variation(&values, DispersionEstimator::Sample);
// }
