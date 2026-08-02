//! Two published methods over one folder of trials, and how far apart they are.

mod common;

use common::{
    analysis_request, bound_request, committed_format, copy_committed_fixtures, declared_pattern,
    registry, synthetic_format, tempdir,
};
use plateforce_batch::agreement::{
    bland_altman, bound_statistic_ids, correlation_with_limits, guard_same_repetition, olp,
    pairs_from,
};
use plateforce_batch::{
    analyse, bind_statistic, compare, AgreementRefusal, BatchCompareRequest, BatchRequest,
    LimitsRequest, TrialIdentity, TrialSet,
};
use plateforce_core::DispersionEstimator;

const HEIGHT: &str = "jump_height_from_takeoff_meters";
const TWO_ONSET_RULES: [&str; 2] = [
    "onset.threshold.noise_relative",
    "onset.threshold.relative_to_system_weight",
];

fn compare_request() -> BatchCompareRequest {
    BatchCompareRequest {
        analysis: BatchRequest::new(analysis_request(1.0)).resolving(&[
            "system_weight",
            "movement_onset",
            "takeoff",
        ]),
        slot: "onset".to_string(),
        method_ids: TWO_ONSET_RULES.iter().map(|id| id.to_string()).collect(),
        quantity: HEIGHT.to_string(),
    }
}

#[test]
fn paired_relation_is_one_row_per_trial_per_method() {
    let directory = tempdir("agreement-paired");
    plateforce_batch::synthetic::write_corpus(&directory, 5, 4, 7).unwrap();
    let set = TrialSet::walk(&directory, &synthetic_format(), &declared_pattern()).unwrap();

    let result = compare(&set, &compare_request());
    println!("{}", result.coverage());
    assert_eq!(result.trial_count, 20);
    assert_eq!(
        result.paired.len(),
        40,
        "two methods over twenty trials, and a variant that failed stays in the denominator"
    );
    assert!(
        result.paired.iter().all(|row| !row.subject.is_empty()),
        "a declared pattern carries the subject onto every paired row"
    );
    std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn pairs_from_one_trace_satisfy_the_design_and_record_it() {
    let directory = tempdir("agreement-one-trace");
    let copied = copy_committed_fixtures(&directory);
    let set = TrialSet::walk(&directory, &committed_format(), &TrialIdentity::FileStem).unwrap();
    let result = compare(&set, &compare_request());

    // One trace in, two methods over it, so every pair comes from one repetition by
    // construction. The guard is satisfied here rather than promised.
    let pairs = pairs_from(&result).expect("the run produced pairs");
    println!("{} pairs from {} trials", pairs.len(), result.trial_count);
    assert_eq!(pairs.len(), copied);
    assert!(bind_statistic("agreement.design.simultaneous_capture").is_some());
    std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn pairs_from_two_files_are_refused_and_named() {
    let left = vec![
        ("subject01_trial1".to_string(), 0.41),
        ("subject01_trial2".to_string(), 0.43),
    ];
    let same = left.clone();
    assert!(guard_same_repetition(&left, &same).is_ok());

    let elsewhere = vec![
        ("subject01_trial1".to_string(), 0.41),
        ("subject01_trial9".to_string(), 0.44),
    ];
    let refusal = guard_same_repetition(&left, &elsewhere)
        .expect_err("agreement across two repetitions is not agreement");
    let message = refusal.message();
    println!("{message}");
    assert!(matches!(
        refusal,
        AgreementRefusal::NotTheSameRepetition { .. }
    ));
    assert!(message.contains("subject01_trial2"), "{message}");
    assert!(message.contains("subject01_trial9"), "{message}");
}

#[test]
fn bland_altman_refuses_when_neither_required_parameter_is_stated() {
    let refusal = LimitsRequest::declared(None, None)
        .expect_err("both are required with no registry default");
    let message = refusal.message();
    println!("{message}");
    assert!(message.contains("unit_of_analysis"), "{message}");
    assert!(message.contains("dispersion"), "{message}");
    assert!(
        message.contains("subject"),
        "the legal values are named: {message}"
    );
    assert!(message.contains("population"), "{message}");
}

#[test]
fn the_subject_unit_of_analysis_needs_a_declared_grouping() {
    let directory = tempdir("agreement-subject-unit");
    let copied = copy_committed_fixtures(&directory);
    let set = TrialSet::walk(&directory, &committed_format(), &TrialIdentity::FileStem).unwrap();
    let result = compare(&set, &compare_request());

    // Taking trials where the design is repeated measures inflates the count and reports a
    // tighter agreement than the data supports, which is why the entry carries the parameter.
    let request = LimitsRequest::declared(Some("subject"), Some("sample")).unwrap();
    let refusal = bland_altman(&set, &result, request).expect_err("no pattern, no subject");
    println!("{}", refusal.message());
    assert_eq!(refusal, AgreementRefusal::SubjectUnitWithoutGrouping);

    let over_trials = LimitsRequest::declared(Some("trial"), Some("sample")).unwrap();
    let limits = bland_altman(&set, &result, over_trials).expect("trials are available");
    println!(
        "bias {:.6} m, limits {:.6} to {:.6}, n = {} of {}",
        limits.bias, limits.lower, limits.upper, limits.n, result.trial_count
    );
    assert_eq!(limits.n, copied);
    std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn a_correlation_arrives_with_its_limits_or_not_at_all() {
    let directory = tempdir("agreement-correlation");
    copy_committed_fixtures(&directory);
    let set = TrialSet::walk(&directory, &committed_format(), &TrialIdentity::FileStem).unwrap();
    let result = compare(&set, &compare_request());
    let pairs = pairs_from(&result).unwrap();

    let together = correlation_with_limits(&pairs, DispersionEstimator::Sample)
        .expect("the pairs support both");
    // There is no accessor for the correlation on its own: the only way out is `both()`, so
    // the refusal is structural rather than a runtime check somebody can route around.
    let (correlation, limits) = together.both();
    println!(
        "r = {correlation:.6}, bias {:.6}, limits {:.6} to {:.6}, n = {}",
        limits.bias,
        limits.lower,
        limits.upper,
        together.n()
    );
    assert_eq!(together.n(), limits.n);
    std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn ordinary_least_products_runs_over_the_same_pairs() {
    let directory = tempdir("agreement-olp");
    let copied = copy_committed_fixtures(&directory);
    let set = TrialSet::walk(&directory, &committed_format(), &TrialIdentity::FileStem).unwrap();
    let result = compare(&set, &compare_request());

    let fit = olp(&result, DispersionEstimator::Sample).expect("the pairs support a fit");
    println!(
        "slope {:.6}, intercept {:.6}, n = {} of {}",
        fit.slope, fit.intercept, fit.n, result.trial_count
    );
    assert_eq!(fit.n, copied);
    assert!(fit.slope.is_finite());
    std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn headline_shape() {
    let directory = tempdir("agreement-headline");
    let copied = copy_committed_fixtures(&directory);
    let set = TrialSet::walk(&directory, &committed_format(), &TrialIdentity::FileStem).unwrap();
    let registry = registry();

    let result = compare(&set, &compare_request());
    let request = LimitsRequest::declared(Some("trial"), Some("sample")).unwrap();
    let limits = bland_altman(&set, &result, request).unwrap();

    // The digest comes from a run over the same set, so the figure names the registry it was
    // taken against rather than resting on a caller's word.
    let analysed = analyse(&set, &bound_request(), &registry).unwrap();

    println!(
        "bias {:.9} m, limits {:.9} to {:.9}, methods {} and {}, digest {}, n = {} of {copied}",
        limits.bias,
        limits.lower,
        limits.upper,
        TWO_ONSET_RULES[0],
        TWO_ONSET_RULES[1],
        analysed.run.registry_digest,
        limits.n
    );

    // The shape and the provenance are asserted, never the value. The number six committed
    // trials produce is not this project's headline, which is a median spread across ten
    // published methods on 244 trials, and asserting one here would be its first failure
    // mode committed in a test.
    assert_eq!(limits.n, copied);
    assert!(limits.lower <= limits.bias && limits.bias <= limits.upper);
    assert!(analysed.run.registry_digest.starts_with("content-"));
    assert_eq!(result.method_ids.len(), 2);
    std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn every_statistic_id_resolves_in_one_table_and_the_registry_carries_it() {
    let registry = registry();
    let ids = bound_statistic_ids();
    let present: Vec<&str> = ids
        .iter()
        .copied()
        .filter(|id| registry.methods.contains_key(*id))
        .collect();
    println!(
        "{} of {} bound statistic ids resolve in the registry",
        present.len(),
        ids.len()
    );
    for id in &ids {
        println!(
            "  {id}  {}",
            if present.contains(id) {
                "resolves"
            } else {
                "no entry"
            }
        );
    }
    // A rule that reported one id when it worked and another when it did not is the defect
    // this table exists to prevent, so every id it holds is bindable.
    for id in &ids {
        assert!(bind_statistic(id).is_some(), "{id} resolves in the table");
    }
    assert!(!present.is_empty(), "the registry carries these entries");
}
