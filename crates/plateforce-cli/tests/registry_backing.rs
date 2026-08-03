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

// `a_rule_the_registry_does_not_file_is_not_reported_as_filed` stood here, and this note is
// where it went rather than an absence a reader has to reconstruct.
//
// It ran the analysis under `onset.threshold.last_within_band` because that id was bound and
// filed under no entry of its own, which is what gave the claim its false side. The terminal no
// longer binds any such rule: a compound name is recorded under the entry it composes, with the
// operator beside it, so every id in a result resolves. Its own control caught the change and
// said so, `every rule this run bound is filed, so it cannot tell a reported entry from a real
// one`, which is a guard reporting that the world moved rather than a guard breaking.
//
// Two things carry what it covered, and between them they cover more.
//
// That the false side is unreachable from here at all is the assertion of
// `every_id_this_build_records_resolves_in_the_registry`, in `plateforce-wasm`. It holds the
// stronger property directly: no id this build records is unfiled, rather than one such id
// existing and being reported honestly.
//
// Where the false side still lives is `registry_backing_follows_the_list_the_engine_is_handed`,
// in `plateforce-analysis`. It withholds exactly one id from `registry_backed_ids` and runs the
// same rule both ways, which reaches what a test at this level cannot: an engine that asserts
// this rather than reading the list it is handed. Withholding one id rather than all of them is
// the point, and it was measured, not assumed. Against an engine answering `!list.is_empty()`,
// an all-or-nothing comparison passes clean and the one-id version fails.
//
// Restoring the test above by reintroducing an unfiled bound rule would be reintroducing the
// defect to satisfy the guard that found it. A reduced-registry fixture was measured and ruled
// out from two directions: dropping one operator entry fails validation with two violations,
// dropping a whole method file with nine, because the registry is a cross-referenced graph.
//
// What remains here is the positive test above, and it is the one that catches the defect
// actually found: a terminal building its backed set from the ids the caller named reports false
// for every operator composed on top, and that test fails naming all eight, kept non-vacuous by
// its own `composed_and_filed > 0`.
