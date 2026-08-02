//! A reduction is a bound method with provenance, never a convenience.

mod common;

use common::{
    bound_request, committed_format, copy_committed_fixtures, declared_pattern, registry,
    synthetic_format, tempdir,
};
use plateforce_batch::{
    aggregate, analyse, with_aggregates, AggregationRefusal, AggregationRequest, AggregationRule,
    GroupKind, TrialIdentity, TrialSet,
};
use plateforce_core::DispersionEstimator;

const HEIGHT: &str = "jump_height_from_takeoff_meters";

fn synthetic(name: &str, subjects: usize, trials: usize) -> (std::path::PathBuf, TrialSet) {
    let directory = tempdir(name);
    plateforce_batch::synthetic::write_corpus(&directory, subjects, trials, 7).unwrap();
    let set = TrialSet::walk(&directory, &synthetic_format(), &declared_pattern()).unwrap();
    (directory, set)
}

fn request(
    rule: &str,
    n: usize,
    kind: GroupKind,
) -> Result<AggregationRequest, AggregationRefusal> {
    AggregationRequest::declared(
        Some(rule),
        Some(n),
        kind,
        vec![HEIGHT.to_string()],
        DispersionEstimator::Sample,
    )
}

#[test]
fn an_aggregate_names_its_rule_and_its_n() {
    let (directory, set) = synthetic("aggregate-named", 5, 5);
    let result = analyse(&set, &bound_request(), &registry()).unwrap();
    assert_eq!(result.coverage.computed, 25, "every trial computed");

    let plan = request("mean_of_best_three_of_at_least_five", 5, GroupKind::Subject).unwrap();
    let joined = with_aggregates(result, &set, &plan).unwrap();

    println!(
        "{} aggregate rows over {} of {} trials",
        joined.aggregates.len(),
        joined.coverage.computed,
        set.len()
    );
    assert_eq!(
        joined.aggregates.len(),
        5,
        "one row per subject per quantity"
    );

    let row = &joined.aggregates[0];
    assert_eq!(row.method_id, "trial.aggregation", "the rule is named");
    assert_eq!(row.n, 3, "and the count it reduced travels with the value");
    assert_eq!(row.group_kind, "subject");
    assert!(row.value.is_some());

    // The chain reaches the rule and the count, so a reader of `aggregates` can see which of
    // three published rules produced the number.
    let chain: Vec<&plateforce_batch::ProvenanceRow> = joined
        .provenance
        .iter()
        .filter(|entry| entry.provenance_id == row.provenance_id)
        .collect();
    assert!(chain
        .iter()
        .any(|entry| entry.parameter == "rule"
            && entry.value == "mean_of_best_three_of_at_least_five"));
    assert!(chain.iter().any(|entry| entry.parameter == "n"));
    println!(
        "chain: {}",
        chain
            .iter()
            .map(|entry| format!("{}={}", entry.parameter, entry.value))
            .collect::<Vec<_>>()
            .join(", ")
    );
    std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn aggregation_without_a_declared_rule_is_refused() {
    let refusal = AggregationRequest::declared(
        None,
        Some(5),
        GroupKind::Subject,
        vec![HEIGHT.to_string()],
        DispersionEstimator::Sample,
    )
    .expect_err("no registry default exists to fall through to");

    let message = refusal.message();
    println!("{message}");
    assert_eq!(refusal, AggregationRefusal::RuleNotStated);
    for published in AggregationRule::PUBLISHED {
        assert!(
            message.contains(published),
            "{published} is named: {message}"
        );
    }
}

#[test]
fn aggregation_without_a_declared_count_is_refused() {
    let refusal = AggregationRequest::declared(
        Some("mean_of_best_two"),
        None,
        GroupKind::Subject,
        vec![HEIGHT.to_string()],
        DispersionEstimator::Sample,
    )
    .expect_err("best of five and best of three are different numbers");
    println!("{}", refusal.message());
    assert_eq!(refusal, AggregationRefusal::CountNotStated);
}

#[test]
fn a_rule_that_ranks_on_a_quantity_this_analysis_does_not_produce_refuses_by_name() {
    let (directory, set) = synthetic("aggregate-peak-force", 3, 3);
    let result = analyse(&set, &bound_request(), &registry()).unwrap();
    let plan = request("best_of_n_by_peak_force", 3, GroupKind::Subject).unwrap();
    let refusal = aggregate(&set, &result, &plan).expect_err("peak force is not among the columns");
    println!("{}", refusal.message());
    assert!(matches!(refusal, AggregationRefusal::QuantityAbsent { .. }));
    assert!(refusal.message().contains("peak_force"));
    std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn a_group_smaller_than_the_rule_requires_names_the_count_it_had_and_the_count_it_needs() {
    let (directory, set) = synthetic("aggregate-too-few", 2, 3);
    let result = analyse(&set, &bound_request(), &registry()).unwrap();
    let plan = request("mean_of_best_three_of_at_least_five", 5, GroupKind::Subject).unwrap();
    let refusal = aggregate(&set, &result, &plan).expect_err("three trials is not five");

    let message = refusal.message();
    println!("{message}");
    assert!(message.contains("3"), "the count it had: {message}");
    assert!(message.contains("5"), "the count it needs: {message}");
    std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn grouping_refusal_names_the_pattern_that_would_supply_a_subject() {
    let directory = tempdir("aggregate-no-grouping");
    copy_committed_fixtures(&directory);
    let set = TrialSet::walk(&directory, &committed_format(), &TrialIdentity::FileStem).unwrap();
    let result = analyse(&set, &bound_request(), &registry()).unwrap();

    let plan = request("mean_of_best_two", 2, GroupKind::Subject).unwrap();
    let refusal = aggregate(&set, &result, &plan).expect_err("no pattern, no subject");
    let message = refusal.message();
    println!("{message}");
    assert!(
        message.contains("{subject}"),
        "it names the placeholder: {message}"
    );

    // The number that is defensible without a subject is still available, and it is a
    // different group kind rather than the same one under a weaker claim.
    let over_the_run = request("mean_of_best_two", 2, GroupKind::Run).unwrap();
    let (rows, _) = aggregate(&set, &result, &over_the_run).expect("a run-level reduction stands");
    println!("{} run-level rows over {} trials", rows.len(), set.len());
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].group_kind, "run");
    std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn a_run_level_reduction_claims_no_published_rule() {
    let directory = tempdir("aggregate-run-level");
    copy_committed_fixtures(&directory);
    let set = TrialSet::walk(&directory, &committed_format(), &TrialIdentity::FileStem).unwrap();
    let result = analyse(&set, &bound_request(), &registry()).unwrap();
    let plan = request("mean_of_best_two", 2, GroupKind::Run).unwrap();
    let (rows, chain) = aggregate(&set, &result, &plan).unwrap();

    // Both entries on the aggregated_value construct are per athlete per session, so a mean
    // across the subjects in a lab section is a descriptive statistic of the user's own set
    // and no registry entry publishes a rule for it.
    println!(
        "run-level method_id {:?}, chain sources {:?}",
        rows[0].method_id,
        chain
            .iter()
            .map(|row| row.source.as_str())
            .collect::<Vec<_>>()
    );
    assert!(
        rows[0].method_id.is_empty(),
        "binding a published rule to arithmetic it does not describe is worse than no entry"
    );
    assert!(chain.iter().all(|row| row.source == "assumed"));
    std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn no_aggregation_writes_the_relations_without_a_fifth() {
    let (directory, set) = synthetic("aggregate-relation-absent", 3, 3);
    let result = analyse(&set, &bound_request(), &registry()).unwrap();
    let out = directory.join("out");
    let written = result.write_csv(&out).unwrap();
    let names: Vec<String> = written
        .iter()
        .map(|p| p.file_name().unwrap().to_str().unwrap().to_string())
        .collect();
    println!("no aggregation: {}", names.join(", "));
    assert!(result.aggregates.is_empty());
    assert!(!out.join("aggregates.csv").exists());
    std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn bound_aggregation_writes_one_more() {
    let (directory, set) = synthetic("aggregate-relation-present", 3, 3);
    let result = analyse(&set, &bound_request(), &registry()).unwrap();
    let plan = request("mean_of_best_two", 2, GroupKind::Subject).unwrap();
    let joined = with_aggregates(result, &set, &plan).unwrap();

    let out = directory.join("out");
    let written = joined.write_csv(&out).unwrap();
    let names: Vec<String> = written
        .iter()
        .map(|p| p.file_name().unwrap().to_str().unwrap().to_string())
        .collect();
    println!("bound aggregation: {}", names.join(", "));
    assert!(out.join("aggregates.csv").exists());

    // `results` gains nothing and loses nothing: the mean is not a row inside it.
    let results = std::fs::read_to_string(out.join("results.csv")).unwrap();
    assert_eq!(
        results.lines().count(),
        1 + joined.results.len(),
        "the header and one row per trial, and no summary row among them"
    );
    std::fs::remove_dir_all(&directory).ok();
}
