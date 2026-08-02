//! What the registry says can be computed, and what stands in the way of the rest.
//!
//! Two numbers that move in opposite directions. A construct leaves `reachable` when
//! somebody classifies the barrier in front of it, so that count falls as the honest work
//! lands. A construct joins `computed` when a rule is bound for it, so that count only
//! rises, and it is the one a floor can be held to.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn plateforce(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_plateforce"))
        .args(arguments)
        .env("NO_COLOR", "1")
        .env("TERM", "dumb")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("the built binary runs")
}

fn reach_in(registry: &str) -> serde_json::Value {
    let output = plateforce(&["--registry", registry, "--format", "json", "reach"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let document: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("the report is one document");
    document["ok"].clone()
}

fn shipped() -> serde_json::Value {
    reach_in("../../registry")
}

/// Every row that cannot be reached names a barrier the operator can act on: a movement they
/// have not recorded, an instrument they do not own, a rule nobody can obtain, or an open
/// question with the query that would settle it.
#[test]
fn every_construct_out_of_reach_names_what_stands_in_the_way() {
    let report = shipped();
    let rows = report["constructs"]
        .as_array()
        .expect("one row per construct");
    let named = ["movement", "instrument", "rule", "undetermined"];

    assert_eq!(
        rows.len(),
        report["construct_count"].as_u64().unwrap() as usize
    );
    let unnamed: Vec<&serde_json::Value> = rows
        .iter()
        .filter(|row| !row["reachable"].as_bool().unwrap())
        .filter(|row| {
            let barriers = row["boundary"].as_array().expect("a barrier list");
            barriers.is_empty()
                || !barriers
                    .iter()
                    .all(|barrier| named.contains(&barrier.as_str().unwrap_or("")))
        })
        .collect();
    println!(
        "constructs reported: {} of {}, out of reach: {}, naming no barrier: {}",
        rows.len(),
        report["construct_count"],
        rows.iter()
            .filter(|r| !r["reachable"].as_bool().unwrap())
            .count(),
        unnamed.len()
    );
    assert!(unnamed.is_empty(), "{unnamed:?}");
}

/// A barrier that is a movement and an instrument at once names both. Collapsing it to
/// either would be wrong about its own scope on the second largest class of walled entry.
///
/// Asked of a registry built to hold one rather than of the shipped registry, so the rule is
/// exercised whatever the shipped classification happens to be.
#[test]
fn a_construct_walled_by_two_things_names_both() {
    let (scratch, construct) = walled_registry("both", None);
    let report = reach_in(scratch.to_str().unwrap());
    let both: Vec<&str> = report["constructs"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| {
            let barriers: Vec<&str> = row["boundary"]
                .as_array()
                .unwrap()
                .iter()
                .filter_map(|b| b.as_str())
                .collect();
            barriers.contains(&"movement") && barriers.contains(&"instrument")
        })
        .map(|row| row["id"].as_str().unwrap())
        .collect();
    println!(
        "constructs naming both barriers: {} of {}",
        both.len(),
        report["construct_count"]
    );
    assert_eq!(
        both,
        [construct],
        "the union rule collapsed a two-part barrier"
    );
    let _ = std::fs::remove_dir_all(&scratch);
}

/// Every barrier the registry can file reaches a row, so a fifth spelling cannot arrive and
/// render as nothing.
#[test]
fn each_barrier_the_registry_files_reaches_a_row() {
    let expected = [
        ("protocol", vec!["movement"]),
        ("equipment", vec!["instrument"]),
        ("both", vec!["instrument", "movement"]),
        ("source", vec!["rule"]),
        ("undetermined", vec!["undetermined"]),
    ];
    for (filed, named) in expected {
        let query = (filed == "undetermined").then_some("what would settle it");
        let (scratch, construct) = walled_registry(filed, query);
        let report = reach_in(scratch.to_str().unwrap());
        let row = report["constructs"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["id"] == construct.as_str())
            .expect("the construct the fixture walls")
            .clone();
        let barriers: Vec<&str> = row["boundary"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|b| b.as_str())
            .collect();
        assert_eq!(barriers, named, "{filed} rendered as {barriers:?}");
        assert!(!row["reachable"].as_bool().unwrap(), "{filed}");
        let _ = std::fs::remove_dir_all(&scratch);
    }
    println!("boundaries the registry files, each reaching a named barrier: 5 of 5");
}

/// A registry copy in which every rule for one construct declares a barrier, whatever the
/// shipped classification holds. The block is written the way the registry writes it, before
/// the next entry opens, because a TOML sub-table attaches to the entry above it.
fn walled_registry(boundary: &str, query: Option<&str>) -> (PathBuf, String) {
    let scratch = std::env::temp_dir().join(format!(
        "plateforce-reach-{boundary}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../registry");
    copy_tree(&source, &scratch);

    let file = scratch.join("methods/asymmetry.toml");
    let text = std::fs::read_to_string(&file).expect("the file the fixture walls");
    let mut block = format!("\n[method.reach]\nboundary = \"{boundary}\"\n");
    if let Some(query) = query {
        block.push_str(&format!("query = \"{query}\"\n"));
    }

    // Any block already filed is dropped first, so the fixture states the barrier whether
    // or not the shipped registry carries one and cannot write a duplicate key.
    let mut written = String::new();
    let mut entries = 0;
    let mut inside_a_filed_block = false;
    for line in text.lines() {
        if line.trim() == "[method.reach]" {
            inside_a_filed_block = true;
            continue;
        }
        if inside_a_filed_block {
            if !line.trim_start().starts_with('[') {
                continue;
            }
            inside_a_filed_block = false;
        }
        if line.trim() == "[[method]]" {
            if entries > 0 {
                written.push_str(&block);
            }
            entries += 1;
        }
        written.push_str(line);
        written.push('\n');
    }
    written.push_str(&block);
    assert!(
        entries > 1,
        "the fixture walls a construct with several rules"
    );
    std::fs::write(&file, written).expect("the fixture is written");

    // The construct these rules fill, read out of the file rather than written here, so the
    // fixture cannot name one the registry has renamed.
    let construct = text
        .lines()
        .find_map(|line| line.strip_prefix("construct = "))
        .map(|value| value.trim().trim_matches('"').to_string())
        .expect("the rules name the construct they fill");
    (scratch, construct)
}

/// The count a floor is held to is the one that rises. Declaring a barrier is the honest
/// work and it lowers the other one, so a floor over `reachable_count` would go red for the
/// work it exists to encourage.
#[test]
fn the_floor_holds_the_number_that_only_rises() {
    let report = shipped();
    let floor = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/reach-floor.txt"),
    )
    .expect("the committed floor");
    let stated: Vec<&str> = floor.split_whitespace().collect();
    assert_eq!(
        stated.len(),
        3,
        "the floor reads <computed> of <constructs>"
    );

    let computed = report["computed_count"].as_u64().unwrap();
    let constructs = report["construct_count"].as_u64().unwrap();
    println!(
        "{computed} of {constructs} constructs compute, floor {} of {}",
        stated[0], stated[2]
    );
    assert!(
        computed >= stated[0].parse::<u64>().unwrap(),
        "the count of constructs this build computes fell below the committed floor"
    );
    assert_eq!(
        constructs,
        stated[2].parse::<u64>().unwrap(),
        "the denominator moved without the floor being restated in the same commit"
    );
}

/// The two questions are separate, so a construct can be one and not the other. A registry
/// that declares a barrier in front of every rule for a construct still reports the rules
/// this build binds for it.
#[test]
fn what_a_recording_supports_and_what_this_build_runs_are_counted_apart() {
    let report = shipped();
    let rows = report["constructs"].as_array().unwrap();
    let computed: Vec<&str> = rows
        .iter()
        .filter(|row| row["computed"].as_bool().unwrap())
        .map(|row| row["id"].as_str().unwrap())
        .collect();
    println!(
        "reachable {} of {}, computed {} of {}, entries declaring a boundary {} of {}",
        report["reachable_count"],
        report["construct_count"],
        report["computed_count"],
        report["construct_count"],
        report["entries_declaring_a_boundary"],
        report["computation_entry_count"]
    );
    assert!(!computed.is_empty(), "no construct reports a bound rule");
    assert!(
        report["computed_count"].as_u64() <= report["construct_count"].as_u64(),
        "more constructs compute than exist"
    );
}

/// An undetermined barrier is a fourth state and not a placeholder. No shipped entry leaves
/// a whole construct undetermined today, so the arm is exercised against a registry built
/// for it: a report that collapsed it into out-of-reach would assert a barrier nobody
/// measured, which is the defect this software exists to prevent.
#[test]
fn a_construct_nobody_has_placed_carries_the_query_that_would_settle_it() {
    let (scratch, _) = walled_registry(
        "undetermined",
        Some("whether two of the five auxiliary channels are horizontal forces"),
    );
    let report = reach_in(scratch.to_str().unwrap());
    let undetermined: Vec<&serde_json::Value> = report["constructs"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| {
            row["boundary"]
                .as_array()
                .unwrap()
                .iter()
                .any(|barrier| barrier == "undetermined")
        })
        .collect();
    println!(
        "constructs reported undetermined: {}, each carrying its query: {}",
        undetermined.len(),
        undetermined
            .iter()
            .filter(|row| !row["query"].is_null())
            .count()
    );
    assert!(
        !undetermined.is_empty(),
        "the registry was built to hold an undetermined construct and none was reported"
    );
    for row in &undetermined {
        assert!(
            row["query"].as_str().is_some_and(|q| !q.is_empty()),
            "an undetermined row with no query asserts a barrier nobody measured: {row}"
        );
    }
    let _ = std::fs::remove_dir_all(&scratch);
}

fn copy_tree(source: &Path, destination: &PathBuf) {
    std::fs::create_dir_all(destination).expect("the fixture directory");
    for entry in std::fs::read_dir(source).expect("the registry is readable") {
        let entry = entry.expect("a registry entry");
        let target = destination.join(entry.file_name());
        if entry.file_type().expect("a file type").is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), &target).expect("a registry file is copied");
        }
    }
}
