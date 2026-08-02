//! The parity manifest, checked from the surface that produced it.
//!
//! The shell harness asks every surface; this asks one, so a change to what this binary can
//! do fails a plain `cargo test` rather than waiting for a workflow.

use std::process::Command;

fn reported() -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_plateforce"))
        .args(["capability", "--format", "json"])
        .env("NO_COLOR", "1")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("the built binary runs");
    assert!(output.status.success());
    String::from_utf8(output.stdout).expect("the manifest is UTF-8")
}

fn committed() -> String {
    std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../CAPABILITY.json"
    ))
    .expect("CAPABILITY.json is committed")
}

/// A committed baseline rather than a comparison between surfaces: several surfaces wrong the
/// same way pass a pairwise check, and every change here is a diff a reviewer sees.
#[test]
fn this_surface_reports_what_the_committed_manifest_claims() {
    assert_eq!(
        reported().trim_end(),
        committed().trim_end(),
        "regenerate with scripts/capability.sh --write and audit the diff"
    );
}

#[test]
fn the_manifest_names_one_method_per_rule_this_build_runs() {
    let manifest: serde_json::Value =
        serde_json::from_str(&committed()).expect("the manifest parses");
    let methods = manifest["ok"]["methods"]
        .as_array()
        .expect("methods is an array");
    println!(
        "methods in the manifest: {} of {} bindings",
        methods.len(),
        plateforce_analysis::BINDINGS.len()
    );
    assert_eq!(methods.len(), plateforce_analysis::BINDINGS.len());
}

/// The arrays are what the manifest may hold. An interaction state cannot be one of them, so
/// a surface with no provisional mode is not a surface that fails this gate.
#[test]
fn the_manifest_holds_only_arrays_a_surface_can_answer_for() {
    let manifest: serde_json::Value =
        serde_json::from_str(&committed()).expect("the manifest parses");
    let mut keys: Vec<&str> = manifest["ok"]
        .as_object()
        .expect("the envelope holds an object")
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
    let manifest: serde_json::Value =
        serde_json::from_str(&committed()).expect("the manifest parses");
    let codes = manifest["ok"]["refusal_codes"]
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
