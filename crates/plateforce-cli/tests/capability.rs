//! The parity manifest, checked from the surface that produced it.
//!
//! The shell harness asks every surface; this asks one, so a change to what this binary can
//! do fails a plain `cargo test` rather than waiting for a workflow.

use std::process::Command;

fn reported() -> serde_json::Value {
    let output = Command::new(env!("CARGO_BIN_EXE_plateforce"))
        .args(["capability", "--format", "json"])
        .env("NO_COLOR", "1")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("the built binary runs");
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).expect("the manifest is UTF-8");
    serde_json::from_str::<serde_json::Value>(&text).expect("the manifest parses")["ok"].clone()
}

fn manifest() -> serde_json::Value {
    let text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../CAPABILITY.json"
    ))
    .expect("CAPABILITY.json is committed");
    serde_json::from_str(&text).expect("the manifest parses")
}

/// What this surface answered last time it was asked, rather than a document every surface
/// shares: which entry points a surface dispatches is a fact about that surface.
fn committed() -> serde_json::Value {
    manifest()["surfaces"]["cli"].clone()
}

/// Name the field that moved rather than printing both documents: the arrays run to
/// thousands of characters and a reader comparing them by eye is reading the wrong thing.
fn differences(committed: &serde_json::Value, reported: &serde_json::Value) -> Vec<String> {
    let empty = serde_json::Map::new();
    let left = committed.as_object().unwrap_or(&empty);
    let right = reported.as_object().unwrap_or(&empty);
    let mut keys: Vec<&String> = left.keys().chain(right.keys()).collect();
    keys.sort();
    keys.dedup();
    keys.into_iter()
        .filter(|key| left.get(*key) != right.get(*key))
        .map(|key| match (left.get(key), right.get(key)) {
            (Some(serde_json::Value::Array(was)), Some(serde_json::Value::Array(now))) => {
                let gone: Vec<&serde_json::Value> =
                    was.iter().filter(|item| !now.contains(item)).collect();
                let new: Vec<&serde_json::Value> =
                    now.iter().filter(|item| !was.contains(item)).collect();
                format!("{key}: no longer reports {gone:?}, now reports {new:?}")
            }
            (was, now) => format!("{key}: committed {was:?}, reports {now:?}"),
        })
        .collect()
}

/// A committed record rather than a comparison against another surface: two surfaces wrong
/// the same way agree with each other, and every change here is a diff a reviewer sees.
#[test]
fn this_surface_reports_what_is_committed_for_it() {
    let moved = differences(&committed(), &reported());
    assert!(
        moved.is_empty(),
        "regenerate with scripts/capability.sh --write and audit the diff:\n  {}",
        moved.join("\n  ")
    );
}

#[test]
fn the_manifest_names_one_method_per_rule_this_build_runs() {
    let committed = committed();
    let methods = committed["methods"]
        .as_array()
        .expect("methods is an array");
    println!(
        "methods in the manifest: {} of {} bindings",
        methods.len(),
        plateforce_analysis::BINDINGS.len()
    );
    assert_eq!(methods.len(), plateforce_analysis::BINDINGS.len());
}

/// The arrays are what a surface may be asked for. An interaction state cannot be one of
/// them, so a surface with no provisional mode is not a surface that fails this gate.
#[test]
fn a_surface_answers_for_the_arrays_and_no_others() {
    let committed = committed();
    let mut keys: Vec<&str> = committed
        .as_object()
        .expect("a surface's record holds an object")
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort();
    assert_eq!(
        keys,
        [
            "methods",
            "operations",
            "output_formats",
            "plateforce_version",
            "refusal_codes",
            "schema"
        ]
    );
}

/// Every way this software can decline maps onto one of the three `sysexits` values a shell
/// reads, and this binary never exits 2 for any of them.
///
/// Membership rather than a value per code: which of 64 and 65 a given refusal deserves is a
/// ruling that belongs to whoever owns the variant, and a guard that pinned each one would be
/// asserting an answer it does not own.
#[test]
fn every_refusal_code_carries_an_exit_status_a_shell_reads() {
    let committed = committed();
    let codes = committed["refusal_codes"]
        .as_array()
        .expect("refusal_codes is an array");
    let named: Vec<i64> = codes
        .iter()
        .map(|record| {
            record["exit_code"]
                .as_i64()
                .expect("an exit code is a number")
        })
        .collect();
    let recognised = named
        .iter()
        .filter(|code| matches!(code, 64 | 65 | 78))
        .count();
    println!(
        "refusal codes: {}; carrying a sysexits status: {} of {}",
        codes.len(),
        recognised,
        named.len()
    );
    assert_eq!(codes.len(), plateforce_core::RefusalCode::ALL.len());
    assert_eq!(recognised, named.len(), "{codes:#?}");
    assert!(!named.contains(&2), "this binary never exits 2");
}

/// The one array that is a ruling rather than a measurement. Regenerating it from the
/// surfaces would make it the union of whatever they happen to do, which none could fail.
#[test]
fn every_operation_owed_is_one_this_surface_reaches() {
    let manifest = manifest();
    let required: Vec<&str> = manifest["required_operations"]
        .as_array()
        .expect("required_operations is an array")
        .iter()
        .map(|name| name.as_str().expect("an operation is a name"))
        .collect();
    let here = committed();
    let reached: Vec<&str> = here["operations"]
        .as_array()
        .expect("operations is an array")
        .iter()
        .map(|name| name.as_str().expect("an operation is a name"))
        .collect();
    let unmet: Vec<&&str> = required
        .iter()
        .filter(|name| !reached.contains(name))
        .collect();
    println!(
        "operations owed by every surface: {}; reached here: {} of {}",
        required.len(),
        required.len() - unmet.len(),
        required.len()
    );
    assert!(!required.is_empty(), "an empty obligation asserts nothing");
    assert!(unmet.is_empty(), "unmet: {unmet:?}");
}

/// The one array that cannot be derived from a table, held against the calls that produce
/// it rather than against a list kept beside it.
///
/// A container this binary writes and does not declare is invisible to a comparison against
/// a committed file, because both sides of that comparison come from the declaration. The
/// only thing outside it is the call.
#[test]
fn every_container_this_binary_writes_is_one_it_declares() {
    let source_directory = concat!(env!("CARGO_MANIFEST_DIR"), "/src");
    let mut source = String::new();
    for entry in std::fs::read_dir(source_directory).expect("the crate has sources") {
        let path = entry.expect("a readable entry").path();
        if path.extension().is_some_and(|kind| kind == "rs") {
            source.push_str(&std::fs::read_to_string(&path).expect("a readable source"));
        }
    }

    // A control first: a pattern that finds nothing reports every container as undeclared
    // and a pattern that finds everything reports none, and the two read the same here.
    assert!(
        source.contains("write_csv("),
        "the scan read no source, so its verdict means nothing"
    );

    let calls = [
        ("csv", source.contains("write_csv(")),
        ("parquet", source.contains("write_parquet(")),
        (
            "json",
            source.contains("write_json(") || source.contains("Format::Json"),
        ),
    ];
    let declared: Vec<String> = committed()["output_formats"]
        .as_array()
        .expect("output_formats is an array")
        .iter()
        .map(|name| name.as_str().expect("a container").to_string())
        .collect();

    let mut faults = Vec::new();
    for (container, called) in calls {
        let named = declared.iter().any(|name| name == container);
        if called && !named {
            faults.push(format!("{container} is written and not declared"));
        }
        if named && !called {
            faults.push(format!("{container} is declared and never written"));
        }
    }
    println!(
        "containers declared: {declared:?}; writers found: {:?}",
        calls
            .iter()
            .filter(|(_, called)| *called)
            .map(|(name, _)| *name)
            .collect::<Vec<&str>>()
    );
    assert!(faults.is_empty(), "{faults:?}");
}
