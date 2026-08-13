//! Whether a trial the run says it removed is a trial the run's figures leave out.
//!
//! Applying a gate is the request asking a finding to remove a trial rather than only name
//! it, and the run reports the count it removed. A figure taken over the trials it removed,
//! published beside that count, is worse than one taken over a set nobody described: a reader
//! who checks the record is misled more precisely than one who ignores it.
//!
//! The rule below is `mean_of_best_three_of_at_least_five`, which refuses under five trials.
//! That is what makes this guard able to fail. A rule reporting `n = 2`
//! whatever the population is answers the same on a run that honours exclusions and one that
//! does not, so a probe built on it cannot tell the two apart.

mod common;

use common::{bound_request, declared_pattern, registry, synthetic_format, tempdir};
use plateforce_analysis::AnalysisResponse;
use plateforce_batch::{
    aggregate, analyse, AggregationRefusal, AggregationRequest, AggregationRule, BatchResult,
    GateFinding, GroupKind, TrialSet, ValidityGate,
};
use plateforce_core::DispersionEstimator;

const GATE_ID: &str = "trial.gate.between_trial_agreement.kraska2009";
const QUANTITY: &str = "jump_height_from_takeoff_meters";

/// Six trials for one subject, of which the gate names three. Half rather than all, so a
/// population that ignored the gate and one that dropped everything are different answers.
const TRIALS: usize = 6;
const NAMED_BY_THE_GATE: usize = 3;

/// A gate matching the trials whose id ends in an even digit. It stands for a registry rule
/// rather than implementing one, which is the idiom the population suite already uses.
struct HalfTheTrials;

impl ValidityGate for HalfTheTrials {
    fn method_id(&self) -> &str {
        GATE_ID
    }
    fn examine(&self, trial_id: &str, _response: &AnalysisResponse) -> Option<GateFinding> {
        let digit = trial_id.chars().last()?.to_digit(10)?;
        (digit % 2 == 0).then(|| GateFinding {
            parameter: Some("permitted_deviation_percent".to_string()),
            value: Some(10.0),
            criterion: "matches half the set, for the channel rather than for the rule".to_string(),
        })
    }
}

fn run(apply: bool) -> (std::path::PathBuf, TrialSet, BatchResult) {
    let directory = tempdir(if apply {
        "gate-applied"
    } else {
        "gate-reporting"
    });
    plateforce_batch::synthetic::write_corpus(&directory, 1, TRIALS, 7).unwrap();
    let set = TrialSet::walk(&directory, &synthetic_format(), &declared_pattern()).unwrap();

    let mut request = bound_request().with_gate(Box::new(HalfTheTrials));
    if apply {
        request.gates.apply(GATE_ID);
    }
    let result = analyse(&set, &request, &registry()).expect("every trial computes");
    (directory, set, result)
}

/// The rule's minimum is five trials, so a population of three has to refuse and a population
/// of six cannot.
fn mean_of_best_three() -> AggregationRequest {
    AggregationRequest {
        rule: AggregationRule::MeanOfBestThreeOfAtLeastFive,
        n: 3,
        ranked_by: "reactive_strength_index".to_string(),
        group_kind: GroupKind::Subject,
        quantities: vec![QUANTITY.to_string()],
        dispersion: DispersionEstimator::Sample,
    }
}

#[test]
fn a_figure_is_not_taken_over_the_trials_the_run_says_it_removed() {
    let (directory, set, result) = run(true);
    assert_eq!(result.coverage.computed, TRIALS, "every trial computed");
    assert_eq!(
        result.run.trials_excluded, NAMED_BY_THE_GATE,
        "the run reports what the gate removed"
    );

    let population = result.population();
    println!(
        "run says excluded {} of {}, population holds {}",
        result.run.trials_excluded,
        result.run.trial_count,
        population.len()
    );
    assert_eq!(population.len(), TRIALS - NAMED_BY_THE_GATE);
    for exclusion in result.exclusions.iter().filter(|row| row.applied) {
        assert!(
            !population.contains(&exclusion.trial_id),
            "{} was removed and is still in the population",
            exclusion.trial_id
        );
    }

    // The discriminator. Three trials against a rule that needs five is a refusal, and it is
    // the only outcome that cannot be produced by a run still pooling all six.
    let refusal = aggregate(&set, &result, &mean_of_best_three())
        .expect_err("a rule needing five trials was handed three and produced a number");
    println!("{refusal:?}");
    match refusal {
        AggregationRefusal::TooFewTrials { had, needs, .. } => {
            assert_eq!(had, TRIALS - NAMED_BY_THE_GATE);
            assert_eq!(needs, 5);
        }
        other => panic!("the wrong refusal: {other:?}"),
    }
    std::fs::remove_dir_all(&directory).ok();
}

/// The other half of the same run. A gate that only reports leaves the trial in, so a guard
/// that passed by removing everything a gate ever named would fail here.
#[test]
fn a_gate_that_only_reports_leaves_every_trial_in_the_population() {
    let (directory, set, result) = run(false);
    assert_eq!(
        result.exclusions.len(),
        NAMED_BY_THE_GATE,
        "the gate named them"
    );
    assert_eq!(
        result.exclusions.iter().filter(|row| row.applied).count(),
        0,
        "and removed none"
    );
    assert_eq!(result.run.trials_excluded, 0);

    let population = result.population();
    println!(
        "reporting gate: population holds {} of {TRIALS}",
        population.len()
    );
    assert_eq!(population.len(), TRIALS);

    // Six trials against a rule needing five is a number, so the refusal above is the gate's
    // doing rather than something this fixture would produce anyway.
    let (rows, _) = aggregate(&set, &result, &mean_of_best_three())
        .expect("six trials satisfy a rule that needs five");
    let row = rows.first().expect("one subject, one quantity");
    println!(
        "{} {} n = {} value {:?}",
        row.group_key, row.quantity, row.n, row.value
    );
    assert_eq!(
        row.n, 3,
        "the rule reduces to three whatever the population"
    );
    assert!(row.value.is_some());
    std::fs::remove_dir_all(&directory).ok();
}

/// The reliability figures build their population separately from the aggregates, so a fix in
/// one says nothing about the other.
#[test]
fn a_reliability_figure_leaves_out_the_trials_the_run_removed() {
    let (directory, set, result) = run(true);
    let values = plateforce_batch::agreement::per_subject_values(&set, &result, QUANTITY)
        .expect("the set declares a grouping");
    let counted: usize = values.iter().map(Vec::len).sum();
    println!(
        "reliability sees {counted} values across {} subjects",
        values.len()
    );
    assert_eq!(
        counted,
        TRIALS - NAMED_BY_THE_GATE,
        "a removed trial reached a reliability figure"
    );
    std::fs::remove_dir_all(&directory).ok();
}
