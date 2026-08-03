//! What the terminal says about which of its rules the registry carries.
//!
//! The engine is told rather than asked, so a surface decides this for itself and two
//! surfaces can publish contradictory provenance about one analysis of one file. Asserted as
//! a property against the registry on disk rather than against a list written here, because
//! a list would go stale the first time an operator is composed onto a rule.

use std::collections::{BTreeMap, BTreeSet};
use std::process::{Command, Output};

use plateforce_analysis::{AnalysisRequest, MethodChoice, WeighingChoice};
use plateforce_core::{read_delimited_column, Trial};

const FIXTURE: &str = "../plateforce-conformance/fixtures/subject01_trial1.force.txt";
const REGISTRY: &str = "../../registry";

fn analysed_under(onset: &str) -> serde_json::Value {
    let output: Output = Command::new(env!("CARGO_BIN_EXE_plateforce"))
        .args([
            "--registry",
            REGISTRY,
            "--format",
            "json",
            "analyse",
            FIXTURE,
            "--column",
            "0",
            "--sample-rate-hz",
            "1200",
            "--sentinel",
            "none",
            "--weighing",
            "bwepoch.fixed_window",
            "--set",
            "weighing.duration=1.0",
            "--onset",
            onset,
            "--set",
            "onset.k=5",
            "--takeoff",
            "takeoff.threshold.absolute_force",
            "--set",
            "takeoff.threshold_n=20",
        ])
        .env("NO_COLOR", "1")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("the built binary runs");
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "the terminal writes JSON: {error}\n{}",
            String::from_utf8_lossy(&output.stderr)
        )
    })
}

/// Every id the registry on disk carries, read through the loader the run itself uses.
fn registry_ids() -> BTreeSet<String> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(REGISTRY);
    plateforce_registry::Registry::load(&path)
        .expect("the committed registry loads")
        .methods
        .keys()
        .cloned()
        .collect()
}

/// A rule filed in the registry reports itself as filed there, whether the caller named it
/// or the binding composed it onto the rule the caller named.
///
/// The three ids a caller passes are the easy case and were never wrong. The operators the
/// binding lays on top of them are the case this exists for, so the run is required to have
/// bound more rules than the caller chose before any of it counts.
#[test]
fn every_bound_rule_the_registry_carries_says_the_registry_carries_it() {
    let document = analysed_under("onset.threshold.noise_relative");
    let bound = document["ok"]["bound_methods"]
        .as_array()
        .expect("a result names the rules it bound");

    let carried = registry_ids();
    let chosen = [
        "bwepoch.fixed_window",
        "onset.threshold.noise_relative",
        "takeoff.threshold.absolute_force",
    ];

    let mut composed_and_filed = 0usize;
    let mut misreported = Vec::new();
    for method in bound {
        let id = method["method_id"].as_str().expect("every rule has an id");
        if !carried.contains(id) {
            continue;
        }
        if !chosen.contains(&id) {
            composed_and_filed += 1;
        }
        if method["registry_backed"] != serde_json::Value::Bool(true) {
            misreported.push(id.to_string());
        }
    }

    println!(
        "{} of {} bound rules are filed in the registry, {composed_and_filed} of them composed \
         rather than chosen",
        bound
            .iter()
            .filter(|m| carried.contains(m["method_id"].as_str().unwrap_or_default()))
            .count(),
        bound.len()
    );
    // Without this the assertion below passes on a run that bound only the caller's three,
    // which is the state in which the defect is invisible.
    assert!(
        composed_and_filed > 0,
        "this run composed no registry rule onto the caller's choices, so it cannot see the \
         difference between asking the registry and asking the request"
    );
    assert!(
        misreported.is_empty(),
        "these rules are filed in the registry and the terminal reports them as not: {misreported:?}"
    );
}

/// The trial the committed request names, read the way the terminal reads it.
fn committed_trial() -> Trial {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(FIXTURE);
    let text = std::fs::read_to_string(&path).expect("the committed fixture is on disk");
    let (values, _) = read_delimited_column(&text, '\t', 0).expect("one column of force");
    Trial::new(values, 1200.0).expect("a trace at the rate the corpus was sampled at")
}

/// What one analysis reports about the backing of every rule it bound, under a stated list.
fn backing_under(trial: &Trial, backed: Vec<String>) -> BTreeMap<String, bool> {
    let request = AnalysisRequest {
        weighing: WeighingChoice {
            method_id: "bwepoch.fixed_window".to_string(),
            parameters: BTreeMap::from([("duration".to_string(), 1.0)]),
            ..Default::default()
        },
        onset: MethodChoice {
            method_id: "onset.threshold.noise_relative".to_string(),
            parameters: BTreeMap::from([("k".to_string(), 5.0)]),
            ..Default::default()
        },
        takeoff: MethodChoice {
            method_id: "takeoff.threshold.absolute_force".to_string(),
            parameters: BTreeMap::from([("threshold_n".to_string(), 20.0)]),
            ..Default::default()
        },
        registry_backed_ids: backed,
        ..Default::default()
    };
    plateforce_analysis::run(trial, &request)
        .unwrap_or_else(|refusal| panic!("the committed fixture computes: {refusal:?}"))
        .bound_methods
        .iter()
        .map(|bound| (bound.method_id.clone(), bound.registry_backed))
        .collect()
}

/// The flag follows the list the request carried, so it distinguishes rather than agreeing
/// with whatever it is shown.
///
/// Taken at the request rather than at the command line, because the terminal can no longer
/// produce a false one: every id this build emits resolves in the shipped registry, so a run
/// driven from a command line has nothing unfiled to bind and an assertion over it would
/// agree with anything. Here one trial is analysed twice under one set of rules and the only
/// thing that moves is what the caller said the registry carries.
#[test]
fn the_backing_a_rule_reports_follows_the_list_the_request_carried() {
    let trial = committed_trial();
    let carried = registry_ids();

    let told_nothing = backing_under(&trial, Vec::new());
    let told_everything = backing_under(&trial, carried.iter().cloned().collect());

    assert!(!told_nothing.is_empty(), "the run bound rules to report on");
    assert_eq!(
        told_nothing.keys().collect::<Vec<_>>(),
        told_everything.keys().collect::<Vec<_>>(),
        "the same rules ran either way, so only the stated list differs"
    );

    // A request naming nothing is a request claiming nothing came from the registry, and the
    // record has to say that rather than what the id happens to look like.
    let claimed: Vec<&String> = told_nothing
        .iter()
        .filter(|(_, backed)| **backed)
        .map(|(id, _)| id)
        .collect();
    assert!(
        claimed.is_empty(),
        "the request named no backed id and these reported themselves as entries: {claimed:?}"
    );

    // The true side, per id rather than in bulk, so a rule the registry stops carrying is a
    // failure here rather than a silent pass.
    for (id, backed) in &told_everything {
        assert_eq!(
            *backed,
            carried.contains(id),
            "{id} is {} the registry and reports {backed}",
            if carried.contains(id) { "in" } else { "not in" }
        );
    }

    // Without this the two assertions above both hold on a flag hardwired either way.
    let moved: Vec<&String> = told_nothing
        .keys()
        .filter(|id| told_nothing[*id] != told_everything[*id])
        .collect();
    println!(
        "{} of {} bound rules changed their reported backing when the stated list did",
        moved.len(),
        told_nothing.len()
    );
    assert!(
        !moved.is_empty(),
        "no rule reported differently under the two lists, so this cannot tell a flag that \
         reads the request from one that is written into the answer"
    );
}
