//! A reduction is a bound method with provenance, never a convenience.

mod common;

use common::{
    bound_request, committed_format, copy_committed_fixtures, declared_pattern, registry,
    synthetic_format, tempdir,
};
use plateforce_batch::{
    aggregate, analyse, with_aggregates, AggregationRefusal, AggregationRequest, AggregationRule,
    GroupKind, ProvenanceRow, TrialIdentity, TrialSet,
};
use plateforce_core::DispersionEstimator;

const HEIGHT: &str = "jump_height_from_takeoff_meters";
const FLIGHT_TIME: &str = "flight_time_seconds";
const FLIGHT_TIME_CONSTRUCT: &str = "flight_time";
const PEAK_FORCE: &str = "net_peak_force_newtons";
const PEAK_FORCE_CONSTRUCT: &str = "net_peak_force";
const RSI_CONSTRUCT: &str = "reactive_strength_index";
const TIME_TO_TAKEOFF: &str = "time_to_takeoff_seconds";
const TIME_TO_TAKEOFF_CONSTRUCT: &str = "time_to_takeoff";

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
    let ranked_by = (rule != "best_of_n_by_peak_force").then_some(RSI_CONSTRUCT);
    AggregationRequest::declared(
        Some(rule),
        Some(n),
        ranked_by,
        kind,
        vec![HEIGHT.to_string()],
        DispersionEstimator::Sample,
    )
}

fn add_peak_force_root(result: &mut plateforce_batch::BatchResult) {
    if !result.quantities.iter().any(|key| key == PEAK_FORCE) {
        result.quantities.push(PEAK_FORCE.to_string());
    }
    result.provenance.push(ProvenanceRow {
        provenance_id: "peak-force-root".to_string(),
        quantity: PEAK_FORCE.to_string(),
        depth: 0,
        method_id: "force.peak.net".to_string(),
        parameter: String::new(),
        value: String::new(),
        source: String::new(),
    });
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
    assert_eq!(
        row.n, 5,
        "the count the request named travels with the value"
    );
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
    assert!(chain
        .iter()
        .any(|entry| entry.parameter == "n" && entry.value == "5"));
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
        Some(RSI_CONSTRUCT),
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
        Some(RSI_CONSTRUCT),
        GroupKind::Subject,
        vec![HEIGHT.to_string()],
        DispersionEstimator::Sample,
    )
    .expect_err("best of five and best of three are different numbers");
    println!("{}", refusal.message());
    assert_eq!(refusal, AggregationRefusal::CountNotStated);
}

/// Neither mean rule says what makes one trial better than another. Without that criterion,
/// the word `best` has no answer and the request must stop before it can choose trials.
#[test]
fn mean_rules_without_a_declared_ranking_criterion_are_refused() {
    let rules = [
        ("mean_of_best_two", 2),
        ("mean_of_best_three_of_at_least_five", 5),
    ];
    let mut refused = 0usize;
    for (rule, n) in rules {
        let rejection = AggregationRequest::declared(
            Some(rule),
            Some(n),
            None,
            GroupKind::Subject,
            vec![HEIGHT.to_string()],
            DispersionEstimator::Sample,
        )
        .expect_err("a mean of the best trials needs to say what ranks them");
        assert!(
            rejection.message().contains("ranked_by"),
            "{rule} was refused without naming the choice left open: {}",
            rejection.message()
        );
        refused += 1;
    }
    assert_eq!(refused, 2, "both mean rules were exercised");
}

/// `best_of_n_by_peak_force` carries its own criterion. Changing where the largest peak sits
/// changes which trial is taken, while making the output quantity point the other way cannot.
#[test]
fn best_by_peak_force_selects_the_trial_with_the_largest_peak_force() {
    let (directory, set) = synthetic("aggregate-ranked-by-peak-force", 1, 3);
    let original = analyse(&set, &bound_request(), &registry()).unwrap();
    assert_eq!(
        original.results.len(),
        3,
        "one subject supplied three trials"
    );

    let arrangements = [
        ([300.0, 200.0, 100.0], [1.0, 20.0, 30.0]),
        ([100.0, 200.0, 300.0], [30.0, 20.0, 1.0]),
    ];
    let mut checked = 0usize;
    for (peaks, heights) in arrangements {
        let mut result = original.clone();
        add_peak_force_root(&mut result);
        for (index, row) in result.results.iter_mut().enumerate() {
            row.values
                .insert(PEAK_FORCE.to_string(), Some(peaks[index]));
            row.values.insert(HEIGHT.to_string(), Some(heights[index]));
        }

        let plan = request("best_of_n_by_peak_force", 3, GroupKind::Subject).unwrap();
        let (rows, _) = aggregate(&set, &result, &plan).unwrap();
        assert_eq!(rows.len(), 1, "one subject and one requested quantity");
        assert_eq!(
            rows[0].value,
            Some(1.0),
            "the largest peak force belongs to the 1.0 m row, while the height column points the other way"
        );
        checked += 1;
    }
    assert_eq!(checked, 2, "the largest peak was placed at both ends");
    std::fs::remove_dir_all(&directory).ok();
}

/// The built-in criterion is still a required method parameter. It travels in the chain even
/// though the rule, rather than the caller, supplies its value.
#[test]
fn best_by_peak_force_records_what_ranked_the_trials() {
    let (directory, set) = synthetic("aggregate-records-ranking", 1, 3);
    let mut result = analyse(&set, &bound_request(), &registry()).unwrap();
    add_peak_force_root(&mut result);
    for (index, row) in result.results.iter_mut().enumerate() {
        row.values
            .insert(PEAK_FORCE.to_string(), Some((index + 1) as f64));
    }

    let plan = request("best_of_n_by_peak_force", 3, GroupKind::Subject).unwrap();
    let (rows, chain) = aggregate(&set, &result, &plan).unwrap();
    let provenance_id = &rows[0].provenance_id;
    let ranking: Vec<_> = chain
        .iter()
        .filter(|row| row.provenance_id == *provenance_id && row.parameter == "ranked_by")
        .collect();
    assert_eq!(
        ranking.len(),
        1,
        "one aggregate chain carries one ranked_by parameter"
    );
    assert_eq!(ranking[0].value, PEAK_FORCE_CONSTRUCT);
    std::fs::remove_dir_all(&directory).ok();
}

/// Two criteria put the same three trials in opposite orders. The aggregate follows the
/// stated criterion, and reversing the relation's row order changes neither answer.
#[test]
fn two_ranking_criteria_select_two_different_sets_of_trials() {
    let (directory, set) = synthetic("aggregate-two-ranking-criteria", 1, 3);
    let original = analyse(&set, &bound_request(), &registry()).unwrap();
    assert_eq!(
        original.results.len(),
        3,
        "one subject supplied three trials"
    );

    let mut answers = Vec::new();
    for reverse_rows in [false, true] {
        let mut result = original.clone();
        for (index, row) in result.results.iter_mut().enumerate() {
            row.values
                .insert(HEIGHT.to_string(), Some([10.0, 20.0, 30.0][index]));
            row.values
                .insert(FLIGHT_TIME.to_string(), Some([300.0, 200.0, 100.0][index]));
            row.values.insert(
                TIME_TO_TAKEOFF.to_string(),
                Some([100.0, 200.0, 300.0][index]),
            );
        }
        if reverse_rows {
            result.results.reverse();
        }

        for (ranked_by, expected) in [
            (FLIGHT_TIME_CONSTRUCT, 15.0),
            (TIME_TO_TAKEOFF_CONSTRUCT, 25.0),
        ] {
            let plan = AggregationRequest::declared(
                Some("mean_of_best_two"),
                Some(2),
                Some(ranked_by),
                GroupKind::Subject,
                vec![HEIGHT.to_string()],
                DispersionEstimator::Sample,
            )
            .unwrap();
            let (rows, chain) = aggregate(&set, &result, &plan).unwrap();
            assert_eq!(rows.len(), 1, "one subject and one requested quantity");
            assert_eq!(
                rows[0].value,
                Some(expected),
                "{ranked_by} chose the wrong two trials with reverse_rows = {reverse_rows}"
            );
            let recorded: Vec<&str> = chain
                .iter()
                .filter(|row| row.parameter == "ranked_by")
                .map(|row| row.value.as_str())
                .collect();
            assert_eq!(recorded, vec![ranked_by]);
            answers.push((reverse_rows, ranked_by, expected));
        }
    }
    assert_eq!(answers.len(), 4, "two criteria ran in two row orders");
    std::fs::remove_dir_all(&directory).ok();
}

/// Movement onset sits below several result chains, but only onset time is that construct's
/// own value. Membership in a chain cannot turn every quantity resting on onset into another
/// ranking value.
#[test]
fn the_ranking_value_is_the_constructs_root_and_not_a_dependency() {
    let (directory, set) = synthetic("aggregate-ranking-root", 1, 3);
    let mut result = analyse(&set, &bound_request(), &registry()).unwrap();
    for (index, row) in result.results.iter_mut().enumerate() {
        row.values
            .insert(HEIGHT.to_string(), Some([10.0, 20.0, 30.0][index]));
        row.values.insert(
            "onset_time_seconds".to_string(),
            Some([300.0, 200.0, 100.0][index]),
        );
    }
    let plan = AggregationRequest::declared(
        Some("mean_of_best_two"),
        Some(2),
        Some("movement_onset"),
        GroupKind::Subject,
        vec![HEIGHT.to_string()],
        DispersionEstimator::Sample,
    )
    .unwrap();
    let (rows, _) = aggregate(&set, &result, &plan).unwrap();
    assert_eq!(rows.len(), 1, "one subject and one requested quantity");
    assert_eq!(
        rows[0].value,
        Some(15.0),
        "the onset-time root chose the first two trials while height pointed the other way"
    );
    std::fs::remove_dir_all(&directory).ok();
}

/// `ranked_by` names a construct, and a construct carrying two result values does not silently
/// pick one of them. Net impulse carries both impulse and takeoff velocity, whose orders can
/// differ when measured system mass differs between trials.
#[test]
fn a_ranking_construct_with_two_values_is_refused() {
    let (directory, set) = synthetic("aggregate-ambiguous-ranking", 1, 3);
    let result = analyse(&set, &bound_request(), &registry()).unwrap();
    let plan = AggregationRequest::declared(
        Some("mean_of_best_two"),
        Some(2),
        Some("net_impulse"),
        GroupKind::Subject,
        vec![HEIGHT.to_string()],
        DispersionEstimator::Sample,
    )
    .unwrap();
    let refusal = aggregate(&set, &result, &plan)
        .expect_err("one construct with two values does not identify a ranking value");
    match refusal {
        AggregationRefusal::RankingConstructAmbiguous {
            construct,
            quantities,
            ..
        } => {
            assert_eq!(construct, "net_impulse");
            assert_eq!(
                quantities,
                vec![
                    "net_impulse_newton_seconds".to_string(),
                    "takeoff_velocity_meters_per_second".to_string(),
                ]
            );
        }
        other => panic!("the wrong refusal: {other:?}"),
    }
    std::fs::remove_dir_all(&directory).ok();
}

/// A tie crossing the cutoff leaves the rule unable to say which trial belongs in the
/// selected set. File order is not allowed to make that choice.
#[test]
fn a_ranking_tied_at_the_selection_boundary_is_refused() {
    let (directory, set) = synthetic("aggregate-ranking-tie", 1, 3);
    let mut result = analyse(&set, &bound_request(), &registry()).unwrap();
    add_peak_force_root(&mut result);
    for (index, row) in result.results.iter_mut().enumerate() {
        row.values
            .insert(PEAK_FORCE.to_string(), Some([300.0, 300.0, 100.0][index]));
    }

    let plan = request("best_of_n_by_peak_force", 3, GroupKind::Subject).unwrap();
    let refusal = aggregate(&set, &result, &plan)
        .expect_err("two equal peaks cannot choose one winning trial between them");
    match refusal {
        AggregationRefusal::RankingTiedAtBoundary { quantity, tied, .. } => {
            assert_eq!(quantity, PEAK_FORCE);
            assert_eq!(tied, 2, "two of three trials share the largest peak");
        }
        other => panic!("the wrong refusal: {other:?}"),
    }
    std::fs::remove_dir_all(&directory).ok();
}

/// A result column can exist while one trial carries no value in it. Leaving that trial out
/// would silently change the population the ranking was taken over.
#[test]
fn a_trial_missing_the_ranking_value_is_refused() {
    let (directory, set) = synthetic("aggregate-ranking-value-missing", 1, 3);
    let mut result = analyse(&set, &bound_request(), &registry()).unwrap();
    for (index, row) in result.results.iter_mut().enumerate() {
        row.values.insert(
            FLIGHT_TIME.to_string(),
            [Some(300.0), None, Some(100.0)][index],
        );
    }
    let plan = AggregationRequest::declared(
        Some("mean_of_best_two"),
        Some(2),
        Some(FLIGHT_TIME_CONSTRUCT),
        GroupKind::Subject,
        vec![HEIGHT.to_string()],
        DispersionEstimator::Sample,
    )
    .unwrap();
    let refusal = aggregate(&set, &result, &plan)
        .expect_err("a missing ranking value cannot silently remove its trial");
    match refusal {
        AggregationRefusal::RankingValueAbsent {
            quantity, trial_id, ..
        } => {
            assert_eq!(quantity, FLIGHT_TIME);
            assert!(
                trial_id.ends_with('2'),
                "the refusal names the trial whose ranking value is absent: {trial_id}"
            );
        }
        other => panic!("the wrong refusal: {other:?}"),
    }
    std::fs::remove_dir_all(&directory).ok();
}

/// A selected set with one missing output is not silently reduced to the one value that
/// remains. The row carries no number and still says how many trials the rule selected.
#[test]
fn a_missing_value_inside_the_selected_set_does_not_become_a_smaller_mean() {
    let (directory, set) = synthetic("aggregate-selected-value-missing", 1, 3);
    let mut result = analyse(&set, &bound_request(), &registry()).unwrap();
    for (index, row) in result.results.iter_mut().enumerate() {
        row.values
            .insert(FLIGHT_TIME.to_string(), Some([300.0, 200.0, 100.0][index]));
        row.values
            .insert(HEIGHT.to_string(), [Some(10.0), None, Some(30.0)][index]);
    }
    let plan = AggregationRequest::declared(
        Some("mean_of_best_two"),
        Some(2),
        Some(FLIGHT_TIME_CONSTRUCT),
        GroupKind::Subject,
        vec![HEIGHT.to_string()],
        DispersionEstimator::Sample,
    )
    .unwrap();
    let (rows, _) = aggregate(&set, &result, &plan).unwrap();
    assert_eq!(rows.len(), 1, "one subject and one requested quantity");
    assert_eq!(
        rows[0].value, None,
        "10.0 was not reported as a mean of two"
    );
    assert_eq!(rows[0].dispersion, None);
    assert_eq!(rows[0].n, 2, "the rule selected two of three trials");
    std::fs::remove_dir_all(&directory).ok();
}

/// The folder can hold enough trials while the requested result column does not. Counting
/// files here would let a mean of two be taken over one value, which is a different rule.
#[test]
fn a_mean_of_two_refuses_when_only_one_of_six_values_is_usable() {
    let (directory, set) = synthetic("aggregate-one-usable-value", 1, 6);
    let original = analyse(&set, &bound_request(), &registry()).unwrap();

    for usable_at in [0usize, 5usize] {
        let mut result = original.clone();
        for (index, row) in result.results.iter_mut().enumerate() {
            row.values.insert(
                FLIGHT_TIME.to_string(),
                (index == usable_at).then_some(0.4 + index as f64 / 100.0),
            );
        }
        let plan = AggregationRequest::declared(
            Some("mean_of_best_two"),
            Some(6),
            Some(RSI_CONSTRUCT),
            GroupKind::Subject,
            vec![FLIGHT_TIME.to_string()],
            DispersionEstimator::Sample,
        )
        .unwrap();
        let refusal = aggregate(&set, &result, &plan)
            .expect_err("one usable value cannot answer a mean-of-two rule");
        match refusal {
            AggregationRefusal::TooFewUsableValues {
                quantity,
                usable,
                trials,
                needs,
                ..
            } => {
                assert_eq!(quantity, FLIGHT_TIME);
                assert_eq!(usable, 1, "one of six values is usable");
                assert_eq!(trials, 6, "the usable count carries its denominator");
                assert_eq!(needs, 2, "mean_of_best_two keeps its floor");
            }
            other => panic!("the wrong refusal: {other:?}"),
        }
    }

    // Exact-floor control. A `<=` check would refuse this, while checking the six files
    // instead of the two values would let the cases above through.
    let mut result = original;
    for (index, row) in result.results.iter_mut().enumerate() {
        row.values.insert(
            FLIGHT_TIME.to_string(),
            match index {
                0 => Some(0.4),
                1 => Some(0.6),
                _ => None,
            },
        );
        row.values.insert(
            "reactive_strength_index_modified".to_string(),
            Some((6 - index) as f64),
        );
    }
    let plan = AggregationRequest::declared(
        Some("mean_of_best_two"),
        Some(6),
        Some(RSI_CONSTRUCT),
        GroupKind::Subject,
        vec![FLIGHT_TIME.to_string()],
        DispersionEstimator::Sample,
    )
    .unwrap();
    let (rows, _) = aggregate(&set, &result, &plan).expect("two usable values meet the floor");
    assert_eq!(rows.len(), 1, "one subject and one requested quantity");
    assert_eq!(rows[0].value, Some(0.5), "both usable values contribute");
    assert_eq!(rows[0].n, 6, "the requested count, not two contributors");
    std::fs::remove_dir_all(&directory).ok();
}

/// The count is part of the method request, not a count reconstructed from how many values
/// the rule reduces. One rule takes one winning value and the other takes three, while both
/// must still export the count the caller named.
#[test]
fn requested_n_is_not_replaced_by_the_number_of_contributors() {
    let (directory, set) = synthetic("aggregate-requested-n", 1, 6);
    let original = analyse(&set, &bound_request(), &registry()).unwrap();

    let mut best = original.clone();
    add_peak_force_root(&mut best);
    for (index, row) in best.results.iter_mut().enumerate() {
        row.values
            .insert(PEAK_FORCE.to_string(), Some((index + 1) as f64));
    }
    let best_plan = request("best_of_n_by_peak_force", 6, GroupKind::Subject).unwrap();
    let (best_rows, best_chain) = aggregate(&set, &best, &best_plan).unwrap();
    assert_eq!(
        best_rows[0].n, 6,
        "best of six contributes one and declares six"
    );
    assert!(best_chain
        .iter()
        .any(|entry| entry.parameter == "n" && entry.value == "6"));

    let mean_plan = request("mean_of_best_three_of_at_least_five", 6, GroupKind::Subject).unwrap();
    let (mean_rows, mean_chain) = aggregate(&set, &original, &mean_plan).unwrap();
    assert_eq!(
        mean_rows[0].n, 6,
        "best three of six contributes three and declares six"
    );
    assert!(mean_chain
        .iter()
        .any(|entry| entry.parameter == "n" && entry.value == "6"));
    std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn a_rule_that_ranks_on_a_construct_this_folder_does_not_carry_refuses_by_name() {
    let (directory, set) = synthetic("aggregate-peak-force", 3, 3);
    let result = analyse(&set, &bound_request(), &registry()).unwrap();
    let plan = request("best_of_n_by_peak_force", 3, GroupKind::Subject).unwrap();
    let refusal = aggregate(&set, &result, &plan).expect_err("peak force is not among the columns");
    println!("{}", refusal.message());
    assert!(matches!(
        refusal,
        AggregationRefusal::RankingConstructAbsent { .. }
    ));
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

/// A session is one subject on one occasion, so a subject with two occasions is one subject
/// and two sessions. Keying both on the session reports one athlete's Monday and Tuesday as
/// two athletes, under a label that says subject, and every count that travels with the
/// value is then the occasion's count rather than the athlete's.
#[test]
fn grouping_by_subject_pools_the_occasions_that_grouping_by_session_keeps_apart() {
    let directory = tempdir("subject-versus-session");
    plateforce_batch::synthetic::write_corpus(&directory, 2, 4, 7).unwrap();
    // Two occasions per subject, named in the file so a template can read them.
    for entry in std::fs::read_dir(&directory).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        let Some(rest) = name.strip_prefix("AT") else {
            continue;
        };
        let (subject, trial) = rest.split_once('_').unwrap();
        let number: usize = trial.trim_end_matches(".txt").parse().unwrap();
        let occasion = if number <= 2 { "monday" } else { "tuesday" };
        std::fs::rename(
            &path,
            directory.join(format!("AT{}_{}_{}.txt", subject, occasion, number)),
        )
        .unwrap();
    }

    let identity = TrialIdentity::DeclaredPattern {
        template: String::from("AT{subject}_{occasion}_{trial}"),
    };
    let set = TrialSet::walk(&directory, &synthetic_format(), &identity).unwrap();
    let result = analyse(&set, &bound_request(), &registry()).unwrap();

    let rows_for = |kind: GroupKind| -> Vec<String> {
        let plan = AggregationRequest::declared(
            Some("mean_of_best_two"),
            Some(2),
            Some(RSI_CONSTRUCT),
            kind,
            vec!["jump_height_from_takeoff_meters".to_string()],
            DispersionEstimator::Sample,
        )
        .unwrap();
        let joined = with_aggregates(result.clone(), &set, &plan).unwrap();
        joined
            .aggregates
            .iter()
            .map(|row| format!("{}={}", row.group_kind, row.group_key))
            .collect()
    };

    let by_subject = rows_for(GroupKind::Subject);
    let by_session = rows_for(GroupKind::Session);
    println!("subject: {by_subject:?}");
    println!("session: {by_session:?}");

    assert_eq!(by_subject.len(), 2, "two athletes, two subject rows");
    assert_eq!(by_session.len(), 4, "each of them on two occasions");
    assert!(
        by_subject.iter().all(|row| !row.contains('/')),
        "a subject key names the athlete and not the occasion: {by_subject:?}"
    );
    assert!(
        by_session.iter().all(|row| row.contains('/')),
        "a session key names both: {by_session:?}"
    );
    std::fs::remove_dir_all(&directory).ok();
}
