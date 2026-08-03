//! What the terminal says about which of its rules the registry carries.
//!
//! The engine is told rather than asked, so a surface decides this for itself and two
//! surfaces can publish contradictory provenance about one analysis of one file. Asserted as
//! a property against the registry on disk rather than against a list written here, because
//! a list would go stale the first time an operator is composed onto a rule.

use std::collections::BTreeSet;
use std::process::{Command, Output};

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
