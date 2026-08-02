//! The denominator every published count is taken over.

mod common;

use common::{
    bound_request, committed_format, copy_committed_fixtures, declared_pattern, registry,
    synthetic_format, tempdir,
};
use plateforce_analysis::AnalysisResponse;
use plateforce_batch::exclusions::GateTally;
use plateforce_batch::{analyse, GateFinding, GateRegistry, TrialIdentity, TrialSet, ValidityGate};

/// A gate that matches every trial, so the channel is exercised without this workstream
/// implementing a registry rule it does not own.
struct EveryTrial(&'static str);

/// The same channel, bound to an id read from the registry rather than written here.
struct NamedGate(String);

impl ValidityGate for NamedGate {
    fn method_id(&self) -> &str {
        &self.0
    }
    fn examine(&self, _trial_id: &str, _response: &AnalysisResponse) -> Option<GateFinding> {
        None
    }
}

impl ValidityGate for EveryTrial {
    fn method_id(&self) -> &str {
        self.0
    }
    fn examine(&self, _trial_id: &str, _response: &AnalysisResponse) -> Option<GateFinding> {
        Some(GateFinding {
            parameter: Some("permitted_deviation_percent".to_string()),
            value: Some(10.0),
            criterion: "matches every trial, for the channel rather than for the rule".to_string(),
        })
    }
}

fn synthetic_run(name: &str, subjects: usize, trials: usize) -> (std::path::PathBuf, TrialSet) {
    let directory = tempdir(name);
    plateforce_batch::synthetic::write_corpus(&directory, subjects, trials, 7).unwrap();
    let set = TrialSet::walk(&directory, &synthetic_format(), &declared_pattern()).unwrap();
    (directory, set)
}

#[test]
fn reporting_gate_names_every_trial_and_removes_none() {
    let (directory, set) = synthetic_run("population-reporting", 5, 4);
    let request = bound_request().with_gate(Box::new(EveryTrial(
        "trial.gate.between_trial_agreement.kraska2009",
    )));
    let result = analyse(&set, &request, &registry()).unwrap();

    let would = result.exclusions.len();
    let applied = result.exclusions.iter().filter(|e| e.applied).count();
    println!(
        "computed {} of {}, would exclude {} of {}, applied {} of {}",
        result.coverage.computed,
        set.len(),
        would,
        set.len(),
        applied,
        set.len()
    );
    assert_eq!(result.coverage.computed, 20, "every trial computed");
    assert_eq!(would, 20, "and every one was named by the gate");
    assert_eq!(applied, 0, "and none was removed");
    assert_eq!(result.run.trials_excluded, 0);
    assert_eq!(result.run.gates_reporting, 1);
    assert_eq!(result.run.gates_applied, 0);
    std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn applying_a_gate_is_a_request_field_and_the_count_still_carries_its_denominator() {
    let (directory, set) = synthetic_run("population-applied", 5, 4);
    let id = "trial.gate.between_trial_agreement.kraska2009";
    let request = bound_request()
        .with_gate(Box::new(EveryTrial(id)))
        .applying(id);
    let result = analyse(&set, &request, &registry()).unwrap();

    println!(
        "computed {} of {}, excluded {} of {}",
        result.coverage.computed,
        set.len(),
        result.run.trials_excluded,
        set.len()
    );
    assert_eq!(
        result.coverage.computed, 20,
        "applying a gate removes from a population, it does not stop the analysis"
    );
    assert_eq!(result.run.trials_excluded, 20);
    assert_eq!(result.run.gates_applied, 1);
    result.run.check_invariants().unwrap();
    std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn read_equals_computed_plus_refused() {
    let directory = tempdir("population-invariant-read");
    copy_committed_fixtures(&directory);
    std::fs::write(directory.join("broken.force.txt"), "").unwrap();
    let set = TrialSet::walk(&directory, &committed_format(), &TrialIdentity::FileStem).unwrap();
    let result = analyse(&set, &bound_request(), &registry()).unwrap();

    assert_eq!(
        result.run.trial_count,
        result.run.computed_count + result.run.refusal_count
    );
    result.run.check_invariants().unwrap();
    println!("{}", result.coverage.line());
    std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn excluded_never_exceeds_computed() {
    let (directory, set) = synthetic_run("population-invariant-excluded", 3, 3);
    let id = "qc.transient_peak_count.pedley2023";
    let request = bound_request()
        .with_gate(Box::new(EveryTrial(id)))
        .applying(id);
    let result = analyse(&set, &request, &registry()).unwrap();
    assert!(result.run.trials_excluded <= result.run.computed_count);
    result.run.check_invariants().unwrap();

    // A run that broke it would be the software breaking an invariant it stated, which is a
    // different fault from a rule declining, and it names both sides.
    let mut broken = result.run.clone();
    broken.trials_excluded = broken.computed_count + 1;
    let error = broken.check_invariants().unwrap_err();
    println!("{error}");
    assert!(
        error.contains(&format!("of {}", broken.trial_count)),
        "{error}"
    );
    std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn an_empty_channel_is_the_correct_state_of_a_run_that_bound_no_gate() {
    let (directory, set) = synthetic_run("population-empty-channel", 2, 2);
    let result = analyse(&set, &bound_request(), &registry()).unwrap();
    println!(
        "gates reporting {} of {}, excluded {} of {}",
        result.run.gates_reporting,
        result.run.gates_reporting,
        result.run.trials_excluded,
        set.len()
    );
    assert!(result.exclusions.is_empty());
    assert_eq!(result.run.gates_reporting, 0);
    assert_eq!(result.run.trials_excluded, 0);
    std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn the_baseline_records_what_each_gate_would_remove_and_fails_when_it_moves() {
    let (directory, set) = synthetic_run("population-baseline", 5, 4);
    let mut request = bound_request();
    // Each gate is bound reporting, so the baseline records the shipped default rather than
    // a state a test invented.
    for id in [
        "trial.gate.between_trial_agreement.kraska2009",
        "qc.countermovement_contamination.chavda2020",
        "qc.transient_peak_count.pedley2023",
    ] {
        request = request.with_gate(Box::new(EveryTrial(id)));
    }
    let result = analyse(&set, &request, &registry()).unwrap();
    assert_eq!(result.coverage.computed, 20, "every trial computed");

    let tally = GateRegistry::tally(&result.exclusions, result.coverage.computed);
    let rendered: Vec<String> = tally.iter().map(GateTally::line).collect();
    let produced = format!("{}\n", rendered.join("\n"));
    print!("{produced}");

    let baseline_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/fixtures/population-baseline.txt"
    );
    let baseline = std::fs::read_to_string(baseline_path)
        .unwrap_or_else(|error| panic!("{baseline_path}: {error}"));

    // The research ratchet measures the Python harness and cannot see a Rust-side gate
    // default, so this file is what a default change has to pass through.
    assert_eq!(
        produced.trim_end(),
        baseline.trim_end(),
        "a gate default moved, so regenerate this baseline deliberately and audit the diff"
    );
    std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn the_registration_surface_is_the_one_the_validity_rules_plug_into() {
    // The six gates on the trial_validity construct have two owners and neither is this
    // workstream. What ships here is the channel, and the count is a query rather than a
    // claim: five arrive, one is walled on a trial the corpus does not hold.
    let registry = registry();
    let gates: Vec<String> = registry
        .methods
        .values()
        .filter(|method| method.construct == "trial_validity")
        .map(|method| method.id.clone())
        .collect();
    println!(
        "{} of {} entries on trial_validity are gates",
        gates.len(),
        registry
            .methods
            .values()
            .filter(|m| m.construct == "trial_validity")
            .count()
    );
    for id in &gates {
        println!("  {id}");
    }
    assert!(!gates.is_empty(), "the construct carries gates to fill");

    // A gate is registered against its registry id, and the channel holds no rule of its own.
    let mut channel = GateRegistry::default();
    assert!(channel.is_empty(), "an empty channel is the shipped state");
    channel.register(Box::new(NamedGate(gates[0].clone())));
    assert_eq!(channel.len(), 1);
    assert_eq!(channel.applied_count(), 0, "bound reporting, not applying");
}
